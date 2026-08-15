//! Canonical schemas and records for aggregate Kotodama durable values.
//!
//! Aggregate state is stored under one durable key.  The compiler emits a
//! preorder schema, while the host converts the VM's flattened word table into
//! a canonical Norito record bound to that schema.
use crate::pointer_abi::PointerType;
use iroha_crypto::Hash;
#[cfg(test)]
use norito::core::serialize_to_buffer;
use norito::{
    Decode, Encode,
    core::{
        Archived, DecodeFromSlice, Error as NoritoError, NoritoDeserialize, NoritoSerialize,
        serialize_to_writer,
    },
};
use std::io::{self, Write};
/// Domain separator for hashes binding stored records to exact state schemas.
pub const STATE_VALUE_SCHEMA_HASH_DOMAIN_V1: &[u8] = b"KOTODAMA_STATE_VALUE_SCHEMA_V1\0";
/// Nominal Norito schema name for compiler-emitted durable-value schemas.
pub const STATE_VALUE_SCHEMA_NAME_V1: &str = "iroha.kotodama.StateValueSchemaV1";
/// Nominal Norito schema name for canonical durable-value records.
pub const STATE_VALUE_RECORD_NAME_V1: &str = "iroha.kotodama.StateValueRecordV1";
/// Magic prefix for the flat, stack-safe V1 schema payload.
pub const STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1: [u8; 4] = *b"KSV1";
/// Width of the logical-node count in the flat V1 schema payload.
pub const STATE_VALUE_SCHEMA_NODE_COUNT_BYTES_V1: u8 = 2;
/// Width of each node tag in the flat V1 schema payload.
pub const STATE_VALUE_SCHEMA_NODE_TAG_BYTES_V1: u8 = 1;
/// Width of each scalar-kind tag in the flat V1 schema payload.
pub const STATE_VALUE_SCHEMA_KIND_TAG_BYTES_V1: u8 = 1;
/// Magic prefix for the flat, stack-safe V1 record payload.
pub const STATE_VALUE_RECORD_PAYLOAD_MAGIC_V1: [u8; 4] = *b"KRV1";
/// Canonical Norito header flags for the fixed-width KRV1 byte wrapper.
const STATE_VALUE_RECORD_FRAME_FLAGS_V1: u8 = norito::core::V1_LAYOUT_FLAGS;
/// Width of every atom-stream count in the flat V1 record payload.
pub const STATE_VALUE_RECORD_STREAM_COUNT_BYTES_V1: u8 = 2;
/// Width of every atom tag in the flat V1 record payload.
pub const STATE_VALUE_RECORD_ATOM_TAG_BYTES_V1: u8 = 1;
/// Width of every pointer byte length in the flat V1 record payload.
pub const STATE_VALUE_RECORD_POINTER_LENGTH_BYTES_V1: u8 = 4;
/// Width of every list item count in the flat V1 record payload.
pub const STATE_VALUE_RECORD_LIST_ITEM_COUNT_BYTES_V1: u8 = 1;
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
/// Maximum complete canonical Norito schema frame accepted by the V1 aggregate-state codec.
pub const MAX_STATE_VALUE_SCHEMA_BYTES: usize = 64 * 1024;
/// Maximum complete canonical Norito record frame accepted by the V1 codec.
pub const MAX_STATE_VALUE_RECORD_BYTES: usize = 1024 * 1024;
/// Minimum capacity accepted for a durable `List<T, N>`.
pub const MIN_STATE_VALUE_LIST_CAPACITY_V1: u8 = 1;
/// Maximum capacity accepted for a durable `List<T, N>`.
pub const MAX_STATE_VALUE_LIST_CAPACITY_V1: u8 = 64;
/// Byte offset of the first aligned word in a decoded state-value table.
pub const DECODED_STATE_VALUE_TABLE_OFFSET: i16 = 8;
/// Width of one decoded state-value word.
pub const DECODED_STATE_VALUE_WORD_BYTES: i16 = 8;
fn state_value_complete_frame_len<T>(payload_len: usize) -> Result<usize, NoritoError> {
    let alignment = norito::core::archived_payload_align::<T>();
    let remainder = norito::core::Header::SIZE % alignment;
    let padding = if remainder == 0 {
        0
    } else {
        alignment - remainder
    };
    norito::core::Header::SIZE
        .checked_add(padding)
        .and_then(|framing| framing.checked_add(norito::core::seq_len_prefix_len(payload_len)))
        .and_then(|framing| framing.checked_add(payload_len))
        .ok_or(NoritoError::LengthMismatch)
}
fn state_value_payload_limit<T>(max_frame_len: usize) -> Result<usize, NoritoError> {
    let empty_frame_len = state_value_complete_frame_len::<T>(0)?;
    max_frame_len
        .checked_sub(empty_frame_len)
        .ok_or(NoritoError::LengthMismatch)
}
fn ensure_state_value_frame_limit<T>(
    payload_len: usize,
    max_frame_len: usize,
    value_name: &'static str,
) -> Result<(), NoritoError> {
    if state_value_complete_frame_len::<T>(payload_len)? > max_frame_len {
        return Err(NoritoError::Message(format!(
            "{value_name} complete canonical Norito frame exceeds {max_frame_len} bytes"
        )));
    }
    Ok(())
}
struct BoundedStateValuePayload {
    bytes: Vec<u8>,
    limit: usize,
    value_name: &'static str,
}
impl BoundedStateValuePayload {
    fn new<T>(max_frame_len: usize, value_name: &'static str) -> Result<Self, NoritoError> {
        Ok(Self {
            bytes: Vec::new(),
            limit: state_value_payload_limit::<T>(max_frame_len)?,
            value_name,
        })
    }
    fn ensure_additional(&self, additional: usize) -> Result<(), NoritoError> {
        let next_len = self
            .bytes
            .len()
            .checked_add(additional)
            .ok_or(NoritoError::LengthMismatch)?;
        if next_len > self.limit {
            return Err(NoritoError::Message(format!(
                "{} complete canonical Norito frame exceeds its byte limit",
                self.value_name
            )));
        }
        Ok(())
    }
    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}
impl Write for BoundedStateValuePayload {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next_len =
            self.bytes.len().checked_add(bytes.len()).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "payload length overflow")
            })?;
        if next_len > self.limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} complete canonical Norito frame exceeds its byte limit",
                    self.value_name
                ),
            ));
        }
        if next_len > self.bytes.capacity() {
            self.bytes
                .try_reserve_exact(next_len - self.bytes.len())
                .map_err(|_| io::Error::other("state-value payload allocation failed"))?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn decode_state_value_payload_wrapper<'a, T>(
    bytes: &'a [u8],
    max_frame_len: usize,
    value_name: &'static str,
) -> Result<(&'a [u8], usize), NoritoError> {
    let (payload_len, prefix_len) = norito::core::inspect_seq_len_slice(bytes)?;
    ensure_state_value_frame_limit::<T>(payload_len, max_frame_len, value_name)?;
    let used = prefix_len
        .checked_add(payload_len)
        .ok_or(NoritoError::LengthMismatch)?;
    let payload = bytes
        .get(prefix_len..used)
        .ok_or(NoritoError::LengthMismatch)?;
    norito::core::note_payload_access(bytes, used);
    Ok((payload, used))
}
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
    const fn from_wire_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::Int,
            1 => Self::Decimal,
            2 => Self::Quantity,
            3 => Self::Bool,
            4 => Self::String,
            5 => Self::Json,
            6 => Self::Bytes,
            7 => Self::AccountId,
            8 => Self::AssetDefinitionId,
            9 => Self::AssetId,
            10 => Self::DomainId,
            11 => Self::NftId,
            12 => Self::Name,
            13 => Self::DataSpaceId,
            14 => Self::AxtDescriptor,
            15 => Self::AssetHandle,
            16 => Self::ProofBlob,
            17 => Self::SoracloudRequest,
            18 => Self::SoracloudResponse,
            _ => return None,
        })
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
///
/// Wire traversal and owned-value cleanup are iterative. The standalone
/// derived codec, `Clone`, `Debug`, and equality implementations on recursive
/// [`StateValueNodeV1`] remain nominal Rust convenience surfaces and are not
/// used at the untrusted aggregate-state boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateValueSchemaV1 {
    /// Preorder aggregate layout.
    pub nodes: Vec<StateValueNodeV1>,
}
impl Drop for StateValueSchemaV1 {
    fn drop(&mut self) {
        let mut pending = Vec::<Box<StateValueSchemaV1>>::new();
        for node in self.nodes.drain(..) {
            if let StateValueNodeV1::List { element, .. } = node {
                pending.push(element);
            }
        }
        while let Some(mut schema) = pending.pop() {
            for node in schema.nodes.drain(..) {
                if let StateValueNodeV1::List { element, .. } = node {
                    pending.push(element);
                }
            }
        }
    }
}
fn state_value_schema_codec_error(message: impl Into<String>) -> NoritoError {
    NoritoError::Message(message.into())
}
fn encode_state_value_schema_payload(schema: &StateValueSchemaV1) -> Result<Vec<u8>, NoritoError> {
    struct Cursor<'a> {
        nodes: &'a [StateValueNodeV1],
        index: usize,
    }
    enum Pending<'a> {
        Start(&'a [StateValueNodeV1]),
        Visit(usize),
        Finish(usize),
    }
    fn insert_cursor<'a>(
        cursors: &mut Vec<Option<Cursor<'a>>>,
        free_cursors: &mut Vec<usize>,
        cursor: Cursor<'a>,
    ) -> usize {
        if let Some(index) = free_cursors.pop() {
            cursors[index] = Some(cursor);
            index
        } else {
            let index = cursors.len();
            cursors.push(Some(cursor));
            index
        }
    }
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let analysis = schema
        .analyze()
        .ok_or_else(|| state_value_schema_codec_error("invalid StateValueSchemaV1"))?;
    let node_count = u16::try_from(analysis.node_count)
        .map_err(|_| state_value_schema_codec_error("StateValueSchemaV1 node count overflow"))?;
    let mut payload = BoundedStateValuePayload::new::<StateValueSchemaV1>(
        MAX_STATE_VALUE_SCHEMA_BYTES,
        "StateValueSchemaV1",
    )?;
    payload.write_all(&STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1)?;
    serialize_to_writer(&node_count, &mut payload)?;
    let mut cursors = Vec::<Option<Cursor<'_>>>::new();
    let mut free_cursors = Vec::new();
    let mut pending = vec![Pending::Start(&schema.nodes)];
    let mut encoded_nodes = 0usize;
    while let Some(item) = pending.pop() {
        match item {
            Pending::Start(nodes) => {
                let cursor =
                    insert_cursor(&mut cursors, &mut free_cursors, Cursor { nodes, index: 0 });
                pending.push(Pending::Finish(cursor));
                pending.push(Pending::Visit(cursor));
            }
            Pending::Visit(cursor) => {
                let cursor_state =
                    cursors
                        .get(cursor)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| {
                            state_value_schema_codec_error(
                                "invalid iterative StateValueSchemaV1 encoder cursor",
                            )
                        })?;
                let nodes = cursor_state.nodes;
                let index = cursor_state.index;
                let node = nodes.get(index).ok_or_else(|| {
                    state_value_schema_codec_error(
                        "truncated preorder stream in StateValueSchemaV1 encoder",
                    )
                })?;
                cursors
                    .get_mut(cursor)
                    .and_then(Option::as_mut)
                    .ok_or_else(|| {
                        state_value_schema_codec_error(
                            "invalid iterative StateValueSchemaV1 encoder cursor",
                        )
                    })?
                    .index = index.checked_add(1).ok_or(NoritoError::LengthMismatch)?;
                encoded_nodes = encoded_nodes
                    .checked_add(1)
                    .ok_or(NoritoError::LengthMismatch)?;
                let node_tag = u8::try_from(node.tag()).map_err(|_| {
                    state_value_schema_codec_error("StateValueSchemaV1 node tag exceeds one byte")
                })?;
                serialize_to_writer(&node_tag, &mut payload)?;
                match node {
                    StateValueNodeV1::Struct { name, fields } => {
                        payload.ensure_additional(
                            name.encoded_len_exact()
                                .ok_or(NoritoError::LengthMismatch)?,
                        )?;
                        serialize_to_writer(name, &mut payload)?;
                        payload.ensure_additional(
                            fields
                                .encoded_len_exact()
                                .ok_or(NoritoError::LengthMismatch)?,
                        )?;
                        serialize_to_writer(fields, &mut payload)?;
                        pending.extend((0..fields.len()).map(|_| Pending::Visit(cursor)));
                    }
                    StateValueNodeV1::Tuple { arity } => {
                        serialize_to_writer(arity, &mut payload)?;
                        pending.extend((0..usize::from(*arity)).map(|_| Pending::Visit(cursor)));
                    }
                    StateValueNodeV1::Option => {
                        pending.push(Pending::Visit(cursor));
                    }
                    StateValueNodeV1::Result => {
                        pending.push(Pending::Visit(cursor));
                        pending.push(Pending::Visit(cursor));
                    }
                    StateValueNodeV1::List { element, capacity } => {
                        serialize_to_writer(capacity, &mut payload)?;
                        pending.push(Pending::Start(&element.nodes));
                    }
                    StateValueNodeV1::Leaf(kind) => {
                        let kind_tag = u8::try_from(kind.tag()).map_err(|_| {
                            state_value_schema_codec_error(
                                "StateValueSchemaV1 leaf tag exceeds one byte",
                            )
                        })?;
                        serialize_to_writer(&kind_tag, &mut payload)?;
                    }
                }
            }
            Pending::Finish(cursor) => {
                let cursor_state =
                    cursors
                        .get_mut(cursor)
                        .and_then(Option::take)
                        .ok_or_else(|| {
                            state_value_schema_codec_error(
                                "invalid iterative StateValueSchemaV1 encoder completion",
                            )
                        })?;
                if cursor_state.index != cursor_state.nodes.len() {
                    return Err(state_value_schema_codec_error(
                        "trailing preorder nodes in StateValueSchemaV1 encoder",
                    ));
                }
                free_cursors.push(cursor);
            }
        }
    }
    if encoded_nodes != analysis.node_count {
        return Err(state_value_schema_codec_error(
            "StateValueSchemaV1 encoder node-count mismatch",
        ));
    }
    let payload = payload.into_inner();
    ensure_state_value_frame_limit::<StateValueSchemaV1>(
        payload.len(),
        MAX_STATE_VALUE_SCHEMA_BYTES,
        "StateValueSchemaV1",
    )?;
    Ok(payload)
}
fn decode_state_value_schema_field<'a, T>(
    encoded: &'a [u8],
    offset: &mut usize,
) -> Result<T, NoritoError>
where
    T: DecodeFromSlice<'a>,
{
    let suffix = encoded.get(*offset..).ok_or(NoritoError::LengthMismatch)?;
    let (value, used) = T::decode_from_slice(suffix)?;
    *offset = offset
        .checked_add(used)
        .ok_or(NoritoError::LengthMismatch)?;
    Ok(value)
}
fn decode_state_value_schema_string(
    encoded: &[u8],
    offset: &mut usize,
) -> Result<String, NoritoError> {
    let suffix = encoded.get(*offset..).ok_or(NoritoError::LengthMismatch)?;
    let (len, prefix_len) = norito::core::read_len_from_slice(suffix)?;
    let used = prefix_len
        .checked_add(len)
        .ok_or(NoritoError::LengthMismatch)?;
    let bytes = suffix
        .get(prefix_len..used)
        .ok_or(NoritoError::LengthMismatch)?;
    let text = std::str::from_utf8(bytes).map_err(|_| NoritoError::InvalidUtf8)?;
    let mut value = String::new();
    value
        .try_reserve_exact(text.len())
        .map_err(|_| NoritoError::AllocationFailed {
            bytes: u64::try_from(text.len()).unwrap_or(u64::MAX),
        })?;
    value.push_str(text);
    *offset = offset
        .checked_add(used)
        .ok_or(NoritoError::LengthMismatch)?;
    Ok(value)
}
fn decode_state_value_schema_strings(
    encoded: &[u8],
    offset: &mut usize,
    max_fields: usize,
) -> Result<Vec<String>, NoritoError> {
    let suffix = encoded.get(*offset..).ok_or(NoritoError::LengthMismatch)?;
    let (field_count, count_prefix_len) = norito::core::read_seq_len_slice(suffix)?;
    if field_count > max_fields || field_count > MAX_STATE_VALUE_NODES {
        return Err(state_value_schema_codec_error(
            "StateValueSchemaV1 field count exceeds the V1 node limit",
        ));
    }
    let mut fields = Vec::new();
    fields
        .try_reserve_exact(field_count)
        .map_err(|_| NoritoError::AllocationFailed {
            bytes: u64::try_from(field_count.saturating_mul(std::mem::size_of::<String>()))
                .unwrap_or(u64::MAX),
        })?;
    let mut used = count_prefix_len;
    for _ in 0..field_count {
        let remaining = suffix.get(used..).ok_or(NoritoError::LengthMismatch)?;
        let (field_len, field_prefix_len) = norito::core::read_len_from_slice(remaining)?;
        let field_used = field_prefix_len
            .checked_add(field_len)
            .ok_or(NoritoError::LengthMismatch)?;
        let field_bytes = remaining
            .get(field_prefix_len..field_used)
            .ok_or(NoritoError::LengthMismatch)?;
        let mut field_offset = 0usize;
        let field = decode_state_value_schema_string(field_bytes, &mut field_offset)?;
        if field_offset != field_bytes.len() {
            return Err(state_value_schema_codec_error(
                "noncanonical StateValueSchemaV1 field-name encoding",
            ));
        }
        fields.push(field);
        used = used
            .checked_add(field_used)
            .ok_or(NoritoError::LengthMismatch)?;
    }
    *offset = offset
        .checked_add(used)
        .ok_or(NoritoError::LengthMismatch)?;
    Ok(fields)
}
fn decode_state_value_schema_payload(encoded: &[u8]) -> Result<StateValueSchemaV1, NoritoError> {
    enum Constructor {
        Struct { name: String, fields: Vec<String> },
        Tuple { arity: u16 },
        Option,
        Result,
        List { capacity: u8 },
    }
    impl Constructor {
        fn child_count(&self) -> usize {
            match self {
                Self::Struct { fields, .. } => fields.len(),
                Self::Tuple { arity } => usize::from(*arity),
                Self::Option | Self::List { .. } => 1,
                Self::Result => 2,
            }
        }
    }
    enum Pending {
        DecodeNode { depth: usize },
        Finish(Constructor),
    }
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    ensure_state_value_frame_limit::<StateValueSchemaV1>(
        encoded.len(),
        MAX_STATE_VALUE_SCHEMA_BYTES,
        "StateValueSchemaV1",
    )?;
    if !encoded.starts_with(&STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1) {
        return Err(state_value_schema_codec_error(
            "invalid StateValueSchemaV1 payload magic",
        ));
    }
    let mut offset = STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1.len();
    let declared_nodes = usize::from(decode_state_value_schema_field::<u16>(
        encoded,
        &mut offset,
    )?);
    if !(1..=MAX_STATE_VALUE_NODES).contains(&declared_nodes) {
        return Err(state_value_schema_codec_error(format!(
            "StateValueSchemaV1 node count must be in 1..={MAX_STATE_VALUE_NODES}"
        )));
    }
    let mut pending = vec![Pending::DecodeNode { depth: 1 }];
    let mut completed = Vec::<StateValueSchemaV1>::new();
    let mut decoded_nodes = 0usize;
    while let Some(item) = pending.pop() {
        match item {
            Pending::DecodeNode { depth } => {
                if depth > MAX_STATE_VALUE_NODES {
                    return Err(state_value_schema_codec_error(
                        "StateValueSchemaV1 nesting exceeds the V1 limit",
                    ));
                }
                decoded_nodes = decoded_nodes
                    .checked_add(1)
                    .ok_or(NoritoError::LengthMismatch)?;
                if decoded_nodes > declared_nodes || decoded_nodes > MAX_STATE_VALUE_NODES {
                    return Err(state_value_schema_codec_error(
                        "StateValueSchemaV1 contains more nodes than declared",
                    ));
                }
                let tag = decode_state_value_schema_field::<u8>(encoded, &mut offset)?;
                let child_depth = depth.checked_add(1).ok_or(NoritoError::LengthMismatch)?;
                let constructor = match u32::from(tag) {
                    StateValueNodeV1::STRUCT_TAG => {
                        let name = decode_state_value_schema_string(encoded, &mut offset)?;
                        let fields = decode_state_value_schema_strings(
                            encoded,
                            &mut offset,
                            declared_nodes,
                        )?;
                        Some(Constructor::Struct { name, fields })
                    }
                    StateValueNodeV1::TUPLE_TAG => {
                        let arity = decode_state_value_schema_field::<u16>(encoded, &mut offset)?;
                        Some(Constructor::Tuple { arity })
                    }
                    StateValueNodeV1::OPTION_TAG => Some(Constructor::Option),
                    StateValueNodeV1::RESULT_TAG => Some(Constructor::Result),
                    StateValueNodeV1::LIST_TAG => {
                        let capacity = decode_state_value_schema_field::<u8>(encoded, &mut offset)?;
                        Some(Constructor::List { capacity })
                    }
                    StateValueNodeV1::LEAF_TAG => {
                        let kind_tag = decode_state_value_schema_field::<u8>(encoded, &mut offset)?;
                        let kind = StateValueKindV1::from_wire_tag(kind_tag).ok_or_else(|| {
                            state_value_schema_codec_error(format!(
                                "invalid StateValueKindV1 wire tag {kind_tag}"
                            ))
                        })?;
                        completed.push(StateValueSchemaV1 {
                            nodes: vec![StateValueNodeV1::Leaf(kind)],
                        });
                        None
                    }
                    other => {
                        return Err(state_value_schema_codec_error(format!(
                            "invalid StateValueNodeV1 wire tag {other}"
                        )));
                    }
                };
                if let Some(constructor) = constructor {
                    let child_count = constructor.child_count();
                    if child_count == 0 || child_count > MAX_STATE_VALUE_NODES {
                        return Err(state_value_schema_codec_error(
                            "invalid StateValueSchemaV1 constructor arity",
                        ));
                    }
                    pending.push(Pending::Finish(constructor));
                    pending.extend(
                        (0..child_count).map(|_| Pending::DecodeNode { depth: child_depth }),
                    );
                }
            }
            Pending::Finish(constructor) => {
                let child_count = constructor.child_count();
                let children_start = completed.len().checked_sub(child_count).ok_or_else(|| {
                    state_value_schema_codec_error(
                        "missing child in iterative StateValueSchemaV1 decoder",
                    )
                })?;
                let mut children = completed.split_off(children_start).into_iter();
                let mut nodes = Vec::new();
                match constructor {
                    Constructor::Struct { name, fields } => {
                        nodes.push(StateValueNodeV1::Struct { name, fields });
                        for mut child in children.by_ref() {
                            nodes.append(&mut child.nodes);
                        }
                    }
                    Constructor::Tuple { arity } => {
                        nodes.push(StateValueNodeV1::Tuple { arity });
                        for mut child in children.by_ref() {
                            nodes.append(&mut child.nodes);
                        }
                    }
                    Constructor::Option => {
                        nodes.push(StateValueNodeV1::Option);
                        let mut child = children.next().ok_or_else(|| {
                            state_value_schema_codec_error(
                                "missing Option child in StateValueSchemaV1 decoder",
                            )
                        })?;
                        nodes.append(&mut child.nodes);
                    }
                    Constructor::Result => {
                        nodes.push(StateValueNodeV1::Result);
                        for mut child in children.by_ref() {
                            nodes.append(&mut child.nodes);
                        }
                    }
                    Constructor::List { capacity } => {
                        let element = children.next().ok_or_else(|| {
                            state_value_schema_codec_error(
                                "missing List element in StateValueSchemaV1 decoder",
                            )
                        })?;
                        nodes.push(StateValueNodeV1::List {
                            element: Box::new(element),
                            capacity,
                        });
                    }
                }
                if children.next().is_some() {
                    return Err(state_value_schema_codec_error(
                        "extra child in iterative StateValueSchemaV1 decoder",
                    ));
                }
                completed.push(StateValueSchemaV1 { nodes });
            }
        }
    }
    if decoded_nodes != declared_nodes || offset != encoded.len() || completed.len() != 1 {
        return Err(state_value_schema_codec_error(
            "noncanonical StateValueSchemaV1 payload shape",
        ));
    }
    let schema = completed.pop().ok_or(NoritoError::LengthMismatch)?;
    if !schema.validate() {
        return Err(state_value_schema_codec_error(
            "decoded StateValueSchemaV1 is invalid",
        ));
    }
    Ok(schema)
}
impl NoritoSerialize for StateValueSchemaV1 {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(STATE_VALUE_SCHEMA_NAME_V1)
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), NoritoError> {
        encode_state_value_schema_payload(self)?.serialize(writer)
    }
}
impl<'a> NoritoDeserialize<'a> for StateValueSchemaV1 {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(STATE_VALUE_SCHEMA_NAME_V1)
    }
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("StateValueSchemaV1 decode")
    }
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, NoritoError> {
        let bytes =
            norito::core::payload_slice_from_ptr(std::ptr::from_ref(archived).cast::<u8>())?;
        let (encoded, _) = decode_state_value_payload_wrapper::<Self>(
            bytes,
            MAX_STATE_VALUE_SCHEMA_BYTES,
            "StateValueSchemaV1",
        )?;
        decode_state_value_schema_payload(encoded)
    }
}
impl<'a> DecodeFromSlice<'a> for StateValueSchemaV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        let (encoded, used) = decode_state_value_payload_wrapper::<Self>(
            bytes,
            MAX_STATE_VALUE_SCHEMA_BYTES,
            "StateValueSchemaV1",
        )?;
        Ok((decode_state_value_schema_payload(encoded)?, used))
    }
}
/// Reconstruct the exact V1 runtime schema for one non-map CNTR durable-state
/// type, including the selected value type of a `StateMap`.
///
/// `StateMap` itself is a host collection resource and therefore is not a
/// persistable value schema. Callers must pass its declared value type. The
/// shared node budget is consumed across recursive [`StateValueNodeV1::List`]
/// element schemas as well as the outer preorder stream.
#[must_use]
pub fn state_value_schema_for_embedded_type_v1(
    ty: &crate::metadata::EmbeddedStateType,
) -> Option<StateValueSchemaV1> {
    use crate::metadata::EmbeddedStateType as Embedded;
    use StateValueKindV1 as Kind;
    enum Pending<'a> {
        Visit {
            ty: &'a Embedded,
            target: usize,
            depth: usize,
        },
        FinishList {
            target: usize,
            element_target: usize,
            capacity: u8,
        },
    }
    let mut node_streams = vec![Vec::new()];
    let mut remaining_nodes = MAX_STATE_VALUE_NODES;
    let mut pending = vec![Pending::Visit {
        ty,
        target: 0,
        depth: 1,
    }];
    while let Some(item) = pending.pop() {
        match item {
            Pending::FinishList {
                target,
                element_target,
                capacity,
            } => {
                let element_nodes = std::mem::take(node_streams.get_mut(element_target)?);
                node_streams.get_mut(target)?.push(StateValueNodeV1::List {
                    element: Box::new(StateValueSchemaV1 {
                        nodes: element_nodes,
                    }),
                    capacity,
                });
            }
            Pending::Visit { ty, target, depth } => {
                if depth > MAX_STATE_VALUE_NODES {
                    return None;
                }
                remaining_nodes = remaining_nodes.checked_sub(1)?;
                let child_depth = depth.checked_add(1)?;
                let nodes = node_streams.get_mut(target)?;
                match ty {
                    Embedded::Int => nodes.push(StateValueNodeV1::Leaf(Kind::Int)),
                    Embedded::Decimal => nodes.push(StateValueNodeV1::Leaf(Kind::Decimal)),
                    Embedded::Quantity => nodes.push(StateValueNodeV1::Leaf(Kind::Quantity)),
                    Embedded::Bool => nodes.push(StateValueNodeV1::Leaf(Kind::Bool)),
                    Embedded::String => nodes.push(StateValueNodeV1::Leaf(Kind::String)),
                    Embedded::Bytes => nodes.push(StateValueNodeV1::Leaf(Kind::Bytes)),
                    Embedded::DataSpaceId => {
                        nodes.push(StateValueNodeV1::Leaf(Kind::DataSpaceId));
                    }
                    Embedded::AccountId => nodes.push(StateValueNodeV1::Leaf(Kind::AccountId)),
                    Embedded::AssetDefinitionId => {
                        nodes.push(StateValueNodeV1::Leaf(Kind::AssetDefinitionId));
                    }
                    Embedded::AssetId => nodes.push(StateValueNodeV1::Leaf(Kind::AssetId)),
                    Embedded::NftId => nodes.push(StateValueNodeV1::Leaf(Kind::NftId)),
                    Embedded::DomainId => nodes.push(StateValueNodeV1::Leaf(Kind::DomainId)),
                    Embedded::Name => nodes.push(StateValueNodeV1::Leaf(Kind::Name)),
                    Embedded::Json => nodes.push(StateValueNodeV1::Leaf(Kind::Json)),
                    Embedded::Tuple(items) => {
                        let arity = u16::try_from(items.len()).ok()?;
                        nodes.push(StateValueNodeV1::Tuple { arity });
                        pending.extend(items.iter().rev().map(|item| Pending::Visit {
                            ty: item,
                            target,
                            depth: child_depth,
                        }));
                    }
                    Embedded::Struct { name, fields } => {
                        nodes.push(StateValueNodeV1::Struct {
                            name: name.clone(),
                            fields: fields.iter().map(|field| field.name.clone()).collect(),
                        });
                        pending.extend(fields.iter().rev().map(|field| Pending::Visit {
                            ty: &field.ty,
                            target,
                            depth: child_depth,
                        }));
                    }
                    Embedded::Option(inner) => {
                        nodes.push(StateValueNodeV1::Option);
                        pending.push(Pending::Visit {
                            ty: inner,
                            target,
                            depth: child_depth,
                        });
                    }
                    Embedded::Result { ok, err } => {
                        nodes.push(StateValueNodeV1::Result);
                        pending.push(Pending::Visit {
                            ty: err,
                            target,
                            depth: child_depth,
                        });
                        pending.push(Pending::Visit {
                            ty: ok,
                            target,
                            depth: child_depth,
                        });
                    }
                    Embedded::List { element, capacity } => {
                        let element_target = node_streams.len();
                        node_streams.push(Vec::new());
                        pending.push(Pending::FinishList {
                            target,
                            element_target,
                            capacity: *capacity,
                        });
                        pending.push(Pending::Visit {
                            ty: element,
                            target: element_target,
                            depth: child_depth,
                        });
                    }
                    Embedded::StateMap { .. } => return None,
                }
            }
        }
    }
    let nodes = node_streams.into_iter().next()?;
    let schema = StateValueSchemaV1 { nodes };
    schema.validate().then_some(schema)
}
/// Reconstruct a CNTR durable-value schema only when every runtime schema
/// bound, including canonical encoded size, is satisfied.
#[must_use]
pub fn admissible_state_value_schema_for_embedded_type_v1(
    ty: &crate::metadata::EmbeddedStateType,
) -> Option<StateValueSchemaV1> {
    let schema = state_value_schema_for_embedded_type_v1(ty)?;
    let encoded = crate::codec::encode_canonical_norito(&schema).ok()?;
    (encoded.len() <= MAX_STATE_VALUE_SCHEMA_BYTES).then_some(schema)
}
impl StateValueSchemaV1 {
    fn analyze(&self) -> Option<StateValueAnalysisV1> {
        #[derive(Clone, Copy)]
        struct Completed {
            analysis: StateValueAnalysisV1,
            next_index: usize,
        }
        enum Pending<'a> {
            Enter {
                nodes: &'a [StateValueNodeV1],
                index: usize,
                depth: usize,
            },
            AggregateNext {
                nodes: &'a [StateValueNodeV1],
                next_index: usize,
                child_depth: usize,
                remaining: usize,
                analysis: StateValueAnalysisV1,
            },
            AggregateMerge {
                nodes: &'a [StateValueNodeV1],
                child_depth: usize,
                remaining: usize,
                analysis: StateValueAnalysisV1,
            },
            FinishOption {
                depth: usize,
            },
            ResultAfterOk {
                nodes: &'a [StateValueNodeV1],
                depth: usize,
            },
            FinishResult {
                depth: usize,
                ok: Completed,
            },
            FinishList {
                parent_next_index: usize,
                parent_depth: usize,
                element_len: usize,
            },
        }
        let mut pending = vec![Pending::Enter {
            nodes: &self.nodes,
            index: 0,
            depth: 1,
        }];
        let mut completed = Vec::<Completed>::new();
        let mut visited_nodes = 0usize;
        while let Some(item) = pending.pop() {
            match item {
                Pending::Enter {
                    nodes,
                    index,
                    depth,
                } => {
                    if depth > MAX_STATE_VALUE_NODES {
                        return None;
                    }
                    visited_nodes = visited_nodes.checked_add(1)?;
                    if visited_nodes > MAX_STATE_VALUE_NODES {
                        return None;
                    }
                    let node = nodes.get(index)?;
                    let next_index = index.checked_add(1)?;
                    let base = StateValueAnalysisV1 {
                        node_count: 1,
                        max_words: 0,
                        depth,
                        contains_resource_handle: false,
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
                            pending.push(Pending::AggregateNext {
                                nodes,
                                next_index,
                                child_depth: depth.checked_add(1)?,
                                remaining: fields.len(),
                                analysis: base,
                            });
                        }
                        StateValueNodeV1::Tuple { arity } => {
                            if *arity < 2 {
                                return None;
                            }
                            pending.push(Pending::AggregateNext {
                                nodes,
                                next_index,
                                child_depth: depth.checked_add(1)?,
                                remaining: usize::from(*arity),
                                analysis: base,
                            });
                        }
                        StateValueNodeV1::Option => {
                            pending.push(Pending::FinishOption { depth });
                            pending.push(Pending::Enter {
                                nodes,
                                index: next_index,
                                depth: depth.checked_add(1)?,
                            });
                        }
                        StateValueNodeV1::Result => {
                            pending.push(Pending::ResultAfterOk { nodes, depth });
                            pending.push(Pending::Enter {
                                nodes,
                                index: next_index,
                                depth: depth.checked_add(1)?,
                            });
                        }
                        StateValueNodeV1::List { element, capacity } => {
                            if !(MIN_STATE_VALUE_LIST_CAPACITY_V1
                                ..=MAX_STATE_VALUE_LIST_CAPACITY_V1)
                                .contains(capacity)
                            {
                                return None;
                            }
                            pending.push(Pending::FinishList {
                                parent_next_index: next_index,
                                parent_depth: depth,
                                element_len: element.nodes.len(),
                            });
                            pending.push(Pending::Enter {
                                nodes: &element.nodes,
                                index: 0,
                                depth: depth.checked_add(1)?,
                            });
                        }
                        StateValueNodeV1::Leaf(kind) => {
                            completed.push(Completed {
                                analysis: StateValueAnalysisV1 {
                                    max_words: 1,
                                    contains_resource_handle: kind.is_resource_handle(),
                                    ..base
                                },
                                next_index,
                            });
                        }
                    }
                }
                Pending::AggregateNext {
                    nodes,
                    next_index,
                    child_depth,
                    remaining,
                    analysis,
                } => {
                    if remaining == 0 {
                        completed.push(Completed {
                            analysis,
                            next_index,
                        });
                    } else {
                        pending.push(Pending::AggregateMerge {
                            nodes,
                            child_depth,
                            remaining: remaining - 1,
                            analysis,
                        });
                        pending.push(Pending::Enter {
                            nodes,
                            index: next_index,
                            depth: child_depth,
                        });
                    }
                }
                Pending::AggregateMerge {
                    nodes,
                    child_depth,
                    remaining,
                    mut analysis,
                } => {
                    let child = completed.pop()?;
                    analysis.node_count =
                        analysis.node_count.checked_add(child.analysis.node_count)?;
                    analysis.max_words =
                        analysis.max_words.checked_add(child.analysis.max_words)?;
                    analysis.depth = analysis.depth.max(child.analysis.depth);
                    analysis.contains_resource_handle |= child.analysis.contains_resource_handle;
                    if analysis.node_count > MAX_STATE_VALUE_NODES
                        || analysis.max_words > MAX_STATE_VALUE_WORDS
                    {
                        return None;
                    }
                    pending.push(Pending::AggregateNext {
                        nodes,
                        next_index: child.next_index,
                        child_depth,
                        remaining,
                        analysis,
                    });
                }
                Pending::FinishOption { depth } => {
                    let child = completed.pop()?;
                    let analysis = StateValueAnalysisV1 {
                        node_count: 1usize.checked_add(child.analysis.node_count)?,
                        max_words: 1,
                        depth: depth.max(child.analysis.depth),
                        contains_resource_handle: child.analysis.contains_resource_handle,
                    };
                    if analysis.node_count > MAX_STATE_VALUE_NODES {
                        return None;
                    }
                    completed.push(Completed {
                        analysis,
                        next_index: child.next_index,
                    });
                }
                Pending::ResultAfterOk { nodes, depth } => {
                    let ok = completed.pop()?;
                    pending.push(Pending::FinishResult { depth, ok });
                    pending.push(Pending::Enter {
                        nodes,
                        index: ok.next_index,
                        depth: depth.checked_add(1)?,
                    });
                }
                Pending::FinishResult { depth, ok } => {
                    let err = completed.pop()?;
                    let analysis = StateValueAnalysisV1 {
                        node_count: 1usize
                            .checked_add(ok.analysis.node_count)?
                            .checked_add(err.analysis.node_count)?,
                        max_words: 1,
                        depth: depth.max(ok.analysis.depth).max(err.analysis.depth),
                        contains_resource_handle: ok.analysis.contains_resource_handle
                            || err.analysis.contains_resource_handle,
                    };
                    if analysis.node_count > MAX_STATE_VALUE_NODES {
                        return None;
                    }
                    completed.push(Completed {
                        analysis,
                        next_index: err.next_index,
                    });
                }
                Pending::FinishList {
                    parent_next_index,
                    parent_depth,
                    element_len,
                } => {
                    let nested = completed.pop()?;
                    if nested.next_index != element_len
                        || nested.analysis.contains_resource_handle
                        || nested.analysis.node_count > MAX_STATE_VALUE_NODES
                        || nested.analysis.max_words > MAX_STATE_VALUE_WORDS
                    {
                        return None;
                    }
                    let analysis = StateValueAnalysisV1 {
                        node_count: 1usize.checked_add(nested.analysis.node_count)?,
                        max_words: 1,
                        depth: parent_depth.max(nested.analysis.depth),
                        contains_resource_handle: false,
                    };
                    if analysis.node_count > MAX_STATE_VALUE_NODES {
                        return None;
                    }
                    completed.push(Completed {
                        analysis,
                        next_index: parent_next_index,
                    });
                }
            }
        }
        if completed.len() != 1 {
            return None;
        }
        let result = completed.pop()?;
        (result.next_index == self.nodes.len()
            && result.analysis.node_count == visited_nodes
            && result.analysis.node_count <= MAX_STATE_VALUE_NODES
            && result.analysis.max_words <= MAX_STATE_VALUE_WORDS
            && result.analysis.depth <= MAX_STATE_VALUE_NODES)
            .then_some(result.analysis)
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
        walk_state_value_atoms(&self.nodes, atoms, false).is_some()
    }
    /// Return actual flattened VM word roles selected by this value.
    pub fn word_kinds_for_atoms(
        &self,
        atoms: &[StateValueAtomV1],
    ) -> Option<Vec<StateValueWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let kinds = walk_state_value_atoms(&self.nodes, atoms, true)?;
        if kinds.len() > MAX_STATE_VALUE_WORDS {
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
    let mut remaining = 1usize;
    while remaining != 0 {
        let Some(node) = nodes.get(*node_index) else {
            return false;
        };
        let Some(next_index) = node_index.checked_add(1) else {
            return false;
        };
        *node_index = next_index;
        remaining -= 1;
        let children = match node {
            StateValueNodeV1::Struct { fields, .. } => fields.len(),
            StateValueNodeV1::Tuple { arity } => usize::from(*arity),
            StateValueNodeV1::Option => 1,
            StateValueNodeV1::Result => 2,
            StateValueNodeV1::List { .. } | StateValueNodeV1::Leaf(_) => 0,
        };
        let Some(next_remaining) = remaining.checked_add(children) else {
            return false;
        };
        remaining = next_remaining;
    }
    true
}
fn max_state_value_word_kinds(
    nodes: &[StateValueNodeV1],
    node_index: &mut usize,
) -> Option<Vec<StateValueWordKindV1>> {
    let mut words = Vec::new();
    let mut pending = vec![true];
    while let Some(record_kind) = pending.pop() {
        let node = nodes.get(*node_index)?;
        *node_index = node_index.checked_add(1)?;
        match node {
            StateValueNodeV1::Struct { fields, .. } => {
                pending.extend((0..fields.len()).map(|_| record_kind));
            }
            StateValueNodeV1::Tuple { arity } => {
                pending.extend((0..usize::from(*arity)).map(|_| record_kind));
            }
            StateValueNodeV1::Option => {
                if record_kind {
                    words.push(StateValueWordKindV1::Sum);
                }
                pending.push(false);
            }
            StateValueNodeV1::Result => {
                if record_kind {
                    words.push(StateValueWordKindV1::Sum);
                }
                pending.push(false);
                pending.push(false);
            }
            StateValueNodeV1::List { .. } => {
                if record_kind {
                    words.push(StateValueWordKindV1::List);
                }
            }
            StateValueNodeV1::Leaf(kind) => {
                if record_kind {
                    words.push(StateValueWordKindV1::Leaf(*kind));
                }
            }
        }
    }
    Some(words)
}
fn walk_state_value_atoms<'a>(
    nodes: &'a [StateValueNodeV1],
    atoms: &'a [StateValueAtomV1],
    record_kinds: bool,
) -> Option<Vec<StateValueWordKindV1>> {
    struct Cursor<'a> {
        nodes: &'a [StateValueNodeV1],
        atoms: &'a [StateValueAtomV1],
        node_index: usize,
        atom_index: usize,
    }
    enum Pending<'a> {
        Visit {
            cursor: usize,
            record_kind: bool,
        },
        Skip {
            cursor: usize,
        },
        ListItems {
            element_nodes: &'a [StateValueNodeV1],
            items: &'a [Vec<StateValueAtomV1>],
            index: usize,
        },
        FinishCursor {
            cursor: usize,
        },
    }
    fn insert_cursor<'a>(
        cursors: &mut Vec<Option<Cursor<'a>>>,
        free_cursors: &mut Vec<usize>,
        cursor: Cursor<'a>,
    ) -> usize {
        if let Some(index) = free_cursors.pop() {
            cursors[index] = Some(cursor);
            index
        } else {
            let index = cursors.len();
            cursors.push(Some(cursor));
            index
        }
    }
    let mut cursors = vec![Some(Cursor {
        nodes,
        atoms,
        node_index: 0,
        atom_index: 0,
    })];
    let mut free_cursors = Vec::new();
    let mut pending = vec![
        Pending::FinishCursor { cursor: 0 },
        Pending::Visit {
            cursor: 0,
            record_kind: record_kinds,
        },
    ];
    let mut kinds = Vec::new();
    while let Some(item) = pending.pop() {
        match item {
            Pending::Visit {
                cursor: cursor_id,
                record_kind,
            } => {
                let cursor = cursors.get_mut(cursor_id)?.as_mut()?;
                let node = cursor.nodes.get(cursor.node_index)?;
                cursor.node_index = cursor.node_index.checked_add(1)?;
                match node {
                    StateValueNodeV1::Struct { fields, .. } => {
                        pending.extend((0..fields.len()).map(|_| Pending::Visit {
                            cursor: cursor_id,
                            record_kind,
                        }));
                    }
                    StateValueNodeV1::Tuple { arity } => {
                        pending.extend((0..usize::from(*arity)).map(|_| Pending::Visit {
                            cursor: cursor_id,
                            record_kind,
                        }));
                    }
                    StateValueNodeV1::Option => {
                        let StateValueAtomV1::Tag(tag) = cursor.atoms.get(cursor.atom_index)?
                        else {
                            return None;
                        };
                        cursor.atom_index = cursor.atom_index.checked_add(1)?;
                        if record_kind {
                            kinds.push(StateValueWordKindV1::Sum);
                        }
                        if *tag {
                            pending.push(Pending::Visit {
                                cursor: cursor_id,
                                record_kind: false,
                            });
                        } else {
                            pending.push(Pending::Skip { cursor: cursor_id });
                        }
                    }
                    StateValueNodeV1::Result => {
                        let StateValueAtomV1::Tag(tag) = cursor.atoms.get(cursor.atom_index)?
                        else {
                            return None;
                        };
                        cursor.atom_index = cursor.atom_index.checked_add(1)?;
                        if record_kind {
                            kinds.push(StateValueWordKindV1::Sum);
                        }
                        if *tag {
                            pending.push(Pending::Skip { cursor: cursor_id });
                            pending.push(Pending::Visit {
                                cursor: cursor_id,
                                record_kind: false,
                            });
                        } else {
                            pending.push(Pending::Visit {
                                cursor: cursor_id,
                                record_kind: false,
                            });
                            pending.push(Pending::Skip { cursor: cursor_id });
                        }
                    }
                    StateValueNodeV1::List { element, capacity } => {
                        let StateValueAtomV1::List(items) = cursor.atoms.get(cursor.atom_index)?
                        else {
                            return None;
                        };
                        cursor.atom_index = cursor.atom_index.checked_add(1)?;
                        if items.len() > usize::from(*capacity) {
                            return None;
                        }
                        if record_kind {
                            kinds.push(StateValueWordKindV1::List);
                        }
                        pending.push(Pending::ListItems {
                            element_nodes: &element.nodes,
                            items,
                            index: 0,
                        });
                    }
                    StateValueNodeV1::Leaf(kind) => {
                        let atom = cursor.atoms.get(cursor.atom_index)?;
                        cursor.atom_index = cursor.atom_index.checked_add(1)?;
                        let valid = matches!(
                            (kind, atom),
                            (StateValueKindV1::Bool, StateValueAtomV1::Bool(_))
                        ) || (kind.is_pointer()
                            && matches!(atom, StateValueAtomV1::Pointer(_)));
                        if !valid {
                            return None;
                        }
                        if record_kind {
                            kinds.push(StateValueWordKindV1::Leaf(*kind));
                        }
                    }
                }
            }
            Pending::Skip { cursor } => {
                let cursor = cursors.get_mut(cursor)?.as_mut()?;
                if !skip_state_value_node(cursor.nodes, &mut cursor.node_index) {
                    return None;
                }
            }
            Pending::ListItems {
                element_nodes,
                items,
                index,
            } => {
                let Some(item) = items.get(index) else {
                    continue;
                };
                pending.push(Pending::ListItems {
                    element_nodes,
                    items,
                    index: index.checked_add(1)?,
                });
                let cursor = insert_cursor(
                    &mut cursors,
                    &mut free_cursors,
                    Cursor {
                        nodes: element_nodes,
                        atoms: item,
                        node_index: 0,
                        atom_index: 0,
                    },
                );
                pending.push(Pending::FinishCursor { cursor });
                pending.push(Pending::Visit {
                    cursor,
                    record_kind: false,
                });
            }
            Pending::FinishCursor { cursor } => {
                let cursor_state = cursors.get_mut(cursor)?.take()?;
                if cursor_state.node_index != cursor_state.nodes.len()
                    || cursor_state.atom_index != cursor_state.atoms.len()
                {
                    return None;
                }
                free_cursors.push(cursor);
            }
        }
    }
    Some(kinds)
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
///
/// Record wire traversal and owner cleanup are iterative. The standalone
/// derived codec, `Clone`, `Debug`, and equality implementations remain
/// recursive nominal Rust convenience surfaces and are not used by the KRV1
/// aggregate-state boundary.
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
fn drop_state_value_atom_streams_iteratively(mut pending: Vec<Vec<StateValueAtomV1>>) {
    while let Some(mut atoms) = pending.pop() {
        for atom in atoms.drain(..) {
            if let StateValueAtomV1::List(mut items) = atom {
                pending.append(&mut items);
            }
        }
    }
}
fn state_value_record_codec_error(message: impl Into<String>) -> NoritoError {
    NoritoError::Message(message.into())
}
fn extend_state_value_record_payload(
    payload: &mut BoundedStateValuePayload,
    bytes: &[u8],
) -> Result<(), NoritoError> {
    payload.write_all(bytes)?;
    Ok(())
}
fn encode_state_value_record_payload(record: &StateValueRecordV1) -> Result<Vec<u8>, NoritoError> {
    enum Pending<'a> {
        Stream {
            atoms: &'a [StateValueAtomV1],
            depth: usize,
        },
        Atom {
            atom: &'a StateValueAtomV1,
            depth: usize,
        },
    }
    let mut payload = BoundedStateValuePayload::new::<StateValueRecordV1>(
        MAX_STATE_VALUE_RECORD_BYTES,
        "StateValueRecordV1",
    )?;
    extend_state_value_record_payload(&mut payload, &STATE_VALUE_RECORD_PAYLOAD_MAGIC_V1)?;
    extend_state_value_record_payload(&mut payload, &record.schema_hash)?;
    let mut pending = vec![Pending::Stream {
        atoms: &record.atoms,
        depth: 0,
    }];
    while let Some(item) = pending.pop() {
        match item {
            Pending::Stream { atoms, depth } => {
                if atoms.is_empty() || atoms.len() > MAX_STATE_VALUE_WORDS {
                    return Err(state_value_record_codec_error(
                        "StateValueRecordV1 atom-stream count is outside 1..=256",
                    ));
                }
                let count = u16::try_from(atoms.len()).map_err(|_| {
                    state_value_record_codec_error("StateValueRecordV1 atom-stream count overflow")
                })?;
                extend_state_value_record_payload(&mut payload, &count.to_le_bytes())?;
                pending.extend(atoms.iter().rev().map(|atom| Pending::Atom { atom, depth }));
            }
            Pending::Atom { atom, depth } => {
                let tag = u8::try_from(atom.tag()).map_err(|_| {
                    state_value_record_codec_error("StateValueRecordV1 atom tag exceeds one byte")
                })?;
                extend_state_value_record_payload(&mut payload, &[tag])?;
                match atom {
                    StateValueAtomV1::Tag(value) | StateValueAtomV1::Bool(value) => {
                        extend_state_value_record_payload(&mut payload, &[u8::from(*value)])?;
                    }
                    StateValueAtomV1::Pointer(bytes) => {
                        let len = u32::try_from(bytes.len()).map_err(|_| {
                            state_value_record_codec_error(
                                "StateValueRecordV1 pointer length overflow",
                            )
                        })?;
                        extend_state_value_record_payload(&mut payload, &len.to_le_bytes())?;
                        extend_state_value_record_payload(&mut payload, bytes)?;
                    }
                    StateValueAtomV1::List(items) => {
                        let child_depth =
                            depth.checked_add(1).ok_or(NoritoError::LengthMismatch)?;
                        if child_depth >= MAX_STATE_VALUE_NODES {
                            return Err(state_value_record_codec_error(
                                "StateValueRecordV1 list nesting exceeds the V1 depth limit",
                            ));
                        }
                        if items.len() > usize::from(MAX_STATE_VALUE_LIST_CAPACITY_V1) {
                            return Err(state_value_record_codec_error(
                                "StateValueRecordV1 list item count exceeds 64",
                            ));
                        }
                        let count = u8::try_from(items.len()).map_err(|_| {
                            state_value_record_codec_error(
                                "StateValueRecordV1 list item count overflow",
                            )
                        })?;
                        extend_state_value_record_payload(&mut payload, &[count])?;
                        if !items.is_empty() {
                            pending.extend(items.iter().rev().map(|atoms| Pending::Stream {
                                atoms,
                                depth: child_depth,
                            }));
                        }
                    }
                }
            }
        }
    }
    let payload = payload.into_inner();
    ensure_state_value_frame_limit::<StateValueRecordV1>(
        payload.len(),
        MAX_STATE_VALUE_RECORD_BYTES,
        "StateValueRecordV1",
    )?;
    Ok(payload)
}
fn take_state_value_record_bytes<'a>(
    encoded: &'a [u8],
    offset: &mut usize,
    len: usize,
) -> Result<&'a [u8], NoritoError> {
    let end = offset.checked_add(len).ok_or(NoritoError::LengthMismatch)?;
    let bytes = encoded
        .get(*offset..end)
        .ok_or(NoritoError::LengthMismatch)?;
    *offset = end;
    Ok(bytes)
}
fn decode_state_value_record_u8(encoded: &[u8], offset: &mut usize) -> Result<u8, NoritoError> {
    take_state_value_record_bytes(encoded, offset, 1)?
        .first()
        .copied()
        .ok_or(NoritoError::LengthMismatch)
}
fn decode_state_value_record_u16(encoded: &[u8], offset: &mut usize) -> Result<u16, NoritoError> {
    Ok(u16::from_le_bytes(
        take_state_value_record_bytes(encoded, offset, 2)?
            .try_into()
            .map_err(|_| NoritoError::LengthMismatch)?,
    ))
}
fn decode_state_value_record_u32(encoded: &[u8], offset: &mut usize) -> Result<u32, NoritoError> {
    Ok(u32::from_le_bytes(
        take_state_value_record_bytes(encoded, offset, 4)?
            .try_into()
            .map_err(|_| NoritoError::LengthMismatch)?,
    ))
}
fn decode_state_value_record_payload(encoded: &[u8]) -> Result<StateValueRecordV1, NoritoError> {
    enum BuilderFrame {
        Stream {
            remaining_atoms: usize,
            atoms: Vec<StateValueAtomV1>,
            depth: usize,
        },
        List {
            remaining_items: usize,
            items: Vec<Vec<StateValueAtomV1>>,
            child_depth: usize,
        },
    }
    struct BuilderFrames(Vec<BuilderFrame>);
    impl BuilderFrames {
        fn ensure_slots(&self, additional: usize) -> Result<(), NoritoError> {
            let required = self
                .0
                .len()
                .checked_add(additional)
                .ok_or(NoritoError::LengthMismatch)?;
            if required > self.0.capacity() {
                return Err(NoritoError::LengthMismatch);
            }
            Ok(())
        }
        fn push(&mut self, frame: BuilderFrame) {
            debug_assert!(self.0.len() < self.0.capacity());
            self.0.push(frame);
        }
    }
    impl Drop for BuilderFrames {
        fn drop(&mut self) {
            let mut current = None;
            loop {
                let frame = if let Some(frame) = current.take() {
                    frame
                } else if let Some(frame) = self.0.pop() {
                    frame
                } else {
                    break;
                };
                match frame {
                    BuilderFrame::Stream { mut atoms, .. } => {
                        let Some(atom) = atoms.pop() else {
                            continue;
                        };
                        if !atoms.is_empty() {
                            debug_assert!(self.0.len() < self.0.capacity());
                            self.0.push(BuilderFrame::Stream {
                                remaining_atoms: 0,
                                atoms,
                                depth: 0,
                            });
                        }
                        if let StateValueAtomV1::List(items) = atom {
                            current = Some(BuilderFrame::List {
                                remaining_items: 0,
                                items,
                                child_depth: 0,
                            });
                        }
                    }
                    BuilderFrame::List { mut items, .. } => {
                        let Some(atoms) = items.pop() else {
                            continue;
                        };
                        if !items.is_empty() {
                            debug_assert!(self.0.len() < self.0.capacity());
                            self.0.push(BuilderFrame::List {
                                remaining_items: 0,
                                items,
                                child_depth: 0,
                            });
                        }
                        current = Some(BuilderFrame::Stream {
                            remaining_atoms: 0,
                            atoms,
                            depth: 0,
                        });
                    }
                }
            }
        }
    }
    fn charge_allocation(
        allocated: &mut usize,
        limit: usize,
        bytes: usize,
    ) -> Result<(), NoritoError> {
        let next = allocated
            .checked_add(bytes)
            .ok_or(NoritoError::LengthMismatch)?;
        if next > limit {
            return Err(state_value_record_codec_error(
                "StateValueRecordV1 decoded allocation exceeds the payload-derived limit",
            ));
        }
        norito::core::reserve_decode_allocation(bytes)?;
        *allocated = next;
        Ok(())
    }
    fn try_vec_with_capacity<T>(
        capacity: usize,
        allocated: &mut usize,
        allocation_limit: usize,
    ) -> Result<Vec<T>, NoritoError> {
        let item_size = std::mem::size_of::<T>();
        if capacity == 0 || item_size == 0 {
            return Ok(Vec::new());
        }
        let requested_bytes = capacity
            .checked_mul(item_size)
            .ok_or(NoritoError::LengthMismatch)?;
        charge_allocation(allocated, allocation_limit, requested_bytes)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| NoritoError::AllocationFailed {
                bytes: u64::try_from(requested_bytes).unwrap_or(u64::MAX),
            })?;
        if values.capacity() > capacity {
            let excess_bytes = values
                .capacity()
                .checked_sub(capacity)
                .and_then(|excess| excess.checked_mul(item_size))
                .ok_or(NoritoError::LengthMismatch)?;
            charge_allocation(allocated, allocation_limit, excess_bytes)?;
        }
        Ok(values)
    }
    fn decode_stream_frame(
        encoded: &[u8],
        offset: &mut usize,
        depth: usize,
        allocated: &mut usize,
        allocation_limit: usize,
    ) -> Result<BuilderFrame, NoritoError> {
        let atom_count = usize::from(decode_state_value_record_u16(encoded, offset)?);
        if atom_count == 0 || atom_count > MAX_STATE_VALUE_WORDS {
            return Err(state_value_record_codec_error(
                "StateValueRecordV1 atom-stream count is outside 1..=256",
            ));
        }
        let atoms =
            try_vec_with_capacity::<StateValueAtomV1>(atom_count, allocated, allocation_limit)?;
        Ok(BuilderFrame::Stream {
            remaining_atoms: atom_count,
            atoms,
            depth,
        })
    }
    fn append_atom(frames: &mut BuilderFrames, atom: StateValueAtomV1) {
        let Some(BuilderFrame::Stream {
            remaining_atoms,
            atoms,
            ..
        }) = frames.0.last_mut()
        else {
            unreachable!("record atom must append to an active stream");
        };
        debug_assert!(*remaining_atoms > 0);
        debug_assert!(atoms.len() < atoms.capacity());
        *remaining_atoms -= 1;
        atoms.push(atom);
    }
    fn append_completed_list(frames: &mut BuilderFrames, items: Vec<Vec<StateValueAtomV1>>) {
        let Some(BuilderFrame::Stream { atoms, .. }) = frames.0.last_mut() else {
            unreachable!("completed record list must append to its parent stream");
        };
        debug_assert!(atoms.len() < atoms.capacity());
        atoms.push(StateValueAtomV1::List(items));
    }
    ensure_state_value_frame_limit::<StateValueRecordV1>(
        encoded.len(),
        MAX_STATE_VALUE_RECORD_BYTES,
        "StateValueRecordV1",
    )?;
    if !encoded.starts_with(&STATE_VALUE_RECORD_PAYLOAD_MAGIC_V1) {
        return Err(state_value_record_codec_error(
            "invalid StateValueRecordV1 payload magic or length",
        ));
    }
    let allocation_limit = encoded
        .len()
        .checked_mul(64)
        .and_then(|bytes| bytes.checked_add(64 * 1024))
        .ok_or(NoritoError::LengthMismatch)?;
    let mut allocated = 0usize;
    let mut offset = STATE_VALUE_RECORD_PAYLOAD_MAGIC_V1.len();
    let schema_hash: [u8; 32] = take_state_value_record_bytes(encoded, &mut offset, 32)?
        .try_into()
        .map_err(|_| NoritoError::LengthMismatch)?;
    const RECORD_PREFIX_BYTES: usize = STATE_VALUE_RECORD_PAYLOAD_MAGIC_V1.len()
        + 32
        + STATE_VALUE_RECORD_STREAM_COUNT_BYTES_V1 as usize;
    const OPEN_LIST_BYTES: usize = STATE_VALUE_RECORD_ATOM_TAG_BYTES_V1 as usize
        + STATE_VALUE_RECORD_LIST_ITEM_COUNT_BYTES_V1 as usize
        + STATE_VALUE_RECORD_STREAM_COUNT_BYTES_V1 as usize;
    let max_open_lists = encoded
        .len()
        .saturating_sub(RECORD_PREFIX_BYTES)
        .checked_div(OPEN_LIST_BYTES)
        .unwrap_or(0)
        .min(MAX_STATE_VALUE_NODES - 1);
    let frame_capacity = max_open_lists
        .checked_mul(2)
        .and_then(|frames| frames.checked_add(1))
        .ok_or(NoritoError::LengthMismatch)?;
    let mut frames = BuilderFrames(try_vec_with_capacity::<BuilderFrame>(
        frame_capacity,
        &mut allocated,
        allocation_limit,
    )?);
    let root = decode_stream_frame(encoded, &mut offset, 0, &mut allocated, allocation_limit)?;
    frames.ensure_slots(1)?;
    frames.push(root);
    loop {
        let stream_complete = matches!(
            frames.0.last(),
            Some(BuilderFrame::Stream {
                remaining_atoms: 0,
                ..
            })
        );
        if stream_complete {
            if frames.0.len() == 1 {
                if offset != encoded.len() {
                    return Err(state_value_record_codec_error(
                        "noncanonical StateValueRecordV1 payload shape",
                    ));
                }
                let root = frames.0.pop().ok_or(NoritoError::LengthMismatch)?;
                let BuilderFrame::Stream { atoms, .. } = root else {
                    unreachable!("record root is always an atom stream");
                };
                return Ok(StateValueRecordV1 { schema_hash, atoms });
            }
            let parent_accepts_stream = matches!(
                frames.0.get(frames.0.len() - 2),
                Some(BuilderFrame::List {
                    remaining_items: 1..,
                    ..
                })
            );
            if !parent_accepts_stream {
                return Err(state_value_record_codec_error(
                    "invalid StateValueRecordV1 list construction",
                ));
            }
            let completed = frames.0.pop().ok_or(NoritoError::LengthMismatch)?;
            let BuilderFrame::Stream { atoms, .. } = completed else {
                unreachable!("completed record child is an atom stream");
            };
            let (remaining_items, child_depth) = {
                let Some(BuilderFrame::List {
                    remaining_items,
                    items,
                    child_depth,
                }) = frames.0.last_mut()
                else {
                    unreachable!("completed record child must have a list parent");
                };
                debug_assert!(*remaining_items > 0);
                debug_assert!(items.len() < items.capacity());
                items.push(atoms);
                *remaining_items -= 1;
                (*remaining_items, *child_depth)
            };
            if remaining_items > 0 {
                let child = decode_stream_frame(
                    encoded,
                    &mut offset,
                    child_depth,
                    &mut allocated,
                    allocation_limit,
                )?;
                frames.ensure_slots(1)?;
                frames.push(child);
                continue;
            }
            let completed = frames.0.pop().ok_or(NoritoError::LengthMismatch)?;
            let BuilderFrame::List { items, .. } = completed else {
                unreachable!("completed record list must be a list frame");
            };
            append_completed_list(&mut frames, items);
            continue;
        }
        let depth = match frames.0.last() {
            Some(BuilderFrame::Stream { depth, .. }) => *depth,
            _ => {
                return Err(state_value_record_codec_error(
                    "invalid StateValueRecordV1 stream construction",
                ));
            }
        };
        let tag = decode_state_value_record_u8(encoded, &mut offset)?;
        match u32::from(tag) {
            StateValueAtomV1::TAG_TAG => {
                let value = match decode_state_value_record_u8(encoded, &mut offset)? {
                    0 => false,
                    1 => true,
                    _ => {
                        return Err(state_value_record_codec_error(
                            "noncanonical StateValueRecordV1 sum tag boolean",
                        ));
                    }
                };
                append_atom(&mut frames, StateValueAtomV1::Tag(value));
            }
            StateValueAtomV1::BOOL_TAG => {
                let value = match decode_state_value_record_u8(encoded, &mut offset)? {
                    0 => false,
                    1 => true,
                    _ => {
                        return Err(state_value_record_codec_error(
                            "noncanonical StateValueRecordV1 boolean",
                        ));
                    }
                };
                append_atom(&mut frames, StateValueAtomV1::Bool(value));
            }
            StateValueAtomV1::POINTER_TAG => {
                let len = usize::try_from(decode_state_value_record_u32(encoded, &mut offset)?)
                    .map_err(|_| NoritoError::LengthMismatch)?;
                let bytes = take_state_value_record_bytes(encoded, &mut offset, len)?;
                let mut pointer =
                    try_vec_with_capacity::<u8>(len, &mut allocated, allocation_limit)?;
                pointer.extend_from_slice(bytes);
                append_atom(&mut frames, StateValueAtomV1::Pointer(pointer));
            }
            StateValueAtomV1::LIST_TAG => {
                let child_depth = depth.checked_add(1).ok_or(NoritoError::LengthMismatch)?;
                if child_depth >= MAX_STATE_VALUE_NODES {
                    return Err(state_value_record_codec_error(
                        "StateValueRecordV1 list nesting exceeds the V1 depth limit",
                    ));
                }
                let item_count = usize::from(decode_state_value_record_u8(encoded, &mut offset)?);
                if item_count > usize::from(MAX_STATE_VALUE_LIST_CAPACITY_V1) {
                    return Err(state_value_record_codec_error(
                        "StateValueRecordV1 list item count exceeds 64",
                    ));
                }
                if item_count == 0 {
                    append_atom(&mut frames, StateValueAtomV1::List(Vec::new()));
                    continue;
                }
                frames.ensure_slots(2)?;
                let items = try_vec_with_capacity::<Vec<StateValueAtomV1>>(
                    item_count,
                    &mut allocated,
                    allocation_limit,
                )?;
                let child = decode_stream_frame(
                    encoded,
                    &mut offset,
                    child_depth,
                    &mut allocated,
                    allocation_limit,
                )?;
                let Some(BuilderFrame::Stream {
                    remaining_atoms, ..
                }) = frames.0.last_mut()
                else {
                    unreachable!("record list must begin inside an atom stream");
                };
                debug_assert!(*remaining_atoms > 0);
                *remaining_atoms -= 1;
                frames.push(BuilderFrame::List {
                    remaining_items: item_count,
                    items,
                    child_depth,
                });
                frames.push(child);
            }
            _ => {
                return Err(state_value_record_codec_error(
                    "unknown StateValueRecordV1 atom tag",
                ));
            }
        }
    }
}
/// Canonical Norito value stored under one aggregate durable-state key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateValueRecordV1 {
    /// Domain-separated hash of the exact encoded schema.
    pub schema_hash: [u8; 32],
    /// Active-only atoms in schema preorder; sum tags select exactly one payload.
    pub atoms: Vec<StateValueAtomV1>,
}
impl Drop for StateValueRecordV1 {
    fn drop(&mut self) {
        drop_state_value_atom_streams_iteratively(vec![std::mem::take(&mut self.atoms)]);
    }
}
impl NoritoSerialize for StateValueRecordV1 {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(STATE_VALUE_RECORD_NAME_V1)
    }
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), NoritoError> {
        encode_state_value_record_payload(self)?.serialize(writer)
    }
}
impl<'a> NoritoDeserialize<'a> for StateValueRecordV1 {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(STATE_VALUE_RECORD_NAME_V1)
    }
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("StateValueRecordV1 decode")
    }
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, NoritoError> {
        let bytes =
            norito::core::payload_slice_from_ptr(std::ptr::from_ref(archived).cast::<u8>())?;
        let (encoded, _) = decode_state_value_payload_wrapper::<Self>(
            bytes,
            MAX_STATE_VALUE_RECORD_BYTES,
            "StateValueRecordV1",
        )?;
        decode_state_value_record_payload(encoded)
    }
}
impl<'a> DecodeFromSlice<'a> for StateValueRecordV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        let (encoded, used) = decode_state_value_payload_wrapper::<Self>(
            bytes,
            MAX_STATE_VALUE_RECORD_BYTES,
            "StateValueRecordV1",
        )?;
        Ok((decode_state_value_record_payload(encoded)?, used))
    }
}
/// Decode one exact canonical V1 durable-state record without re-encoding it.
///
/// The outer Norito frame must use the canonical uncompressed layout, exact
/// type-specific alignment, schema hash, payload length, and checksum. The
/// custom KRV1 payload decoder then enforces its unique flat representation and
/// must consume the complete checksummed payload.
pub fn decode_canonical_state_value_record_v1(
    bytes: &[u8],
) -> Result<StateValueRecordV1, NoritoError> {
    if bytes.len() > MAX_STATE_VALUE_RECORD_BYTES {
        return Err(state_value_record_codec_error(
            "StateValueRecordV1 complete canonical Norito frame exceeds its byte limit",
        ));
    }
    let limits = norito::canonical_decode_limits(bytes.len());
    norito::core::with_decode_limits(limits, || {
        let canonical_flags = STATE_VALUE_RECORD_FRAME_FLAGS_V1;
        let _canonical_flags = norito::core::DecodeFlagsGuard::enter(canonical_flags);
        let _payload_context = norito::core::PayloadCtxGuard::enter(bytes);
        let view = match norito::core::from_bytes_view(bytes) {
            Ok(view) => view,
            Err(
                NoritoError::DecodeFlagsMismatch { .. }
                | NoritoError::UnsupportedCompression { .. },
            ) => return Err(NoritoError::NonCanonicalEncoding),
            Err(error) => return Err(error),
        };
        if view.flags() != canonical_flags {
            return Err(NoritoError::NonCanonicalEncoding);
        }
        view.decode_exact::<StateValueRecordV1>()
    })
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
    fn nested_list_schema(wrappers: usize) -> StateValueSchemaV1 {
        (0..wrappers).fold(
            StateValueSchemaV1 {
                nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Bool)],
            },
            |element, _| StateValueSchemaV1 {
                nodes: vec![StateValueNodeV1::List {
                    element: Box::new(element),
                    capacity: 1,
                }],
            },
        )
    }
    fn drop_schema_iteratively(schema: StateValueSchemaV1) {
        drop(schema);
    }
    fn nested_list_atoms(wrappers: usize) -> Vec<StateValueAtomV1> {
        (0..wrappers).fold(vec![StateValueAtomV1::Bool(true)], |item, _| {
            vec![StateValueAtomV1::List(vec![item])]
        })
    }
    fn drop_atoms_iteratively(atoms: Vec<StateValueAtomV1>) {
        let mut pending = vec![atoms];
        while let Some(mut atoms) = pending.pop() {
            for atom in atoms.drain(..) {
                if let StateValueAtomV1::List(items) = atom {
                    pending.extend(items);
                }
            }
        }
    }
    fn nested_option_schema(wrappers: usize) -> StateValueSchemaV1 {
        let mut nodes = vec![StateValueNodeV1::Option; wrappers];
        nodes.push(StateValueNodeV1::Leaf(StateValueKindV1::Bool));
        StateValueSchemaV1 { nodes }
    }
    fn schema_with_name_len(name_len: usize) -> StateValueSchemaV1 {
        StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Struct {
                    name: "n".repeat(name_len),
                    fields: vec!["x".to_owned()],
                },
                StateValueNodeV1::Leaf(StateValueKindV1::Bool),
            ],
        }
    }
    fn encode_schema_payload_without_limit(name_len: usize) -> Vec<u8> {
        let _canonical_flags =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let mut payload = Vec::new();
        payload.extend_from_slice(&STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1);
        serialize_to_buffer(&2_u16, &mut payload).expect("serialize schema node count");
        serialize_to_buffer(&(StateValueNodeV1::STRUCT_TAG as u8), &mut payload)
            .expect("serialize Struct tag");
        serialize_to_buffer(&"n".repeat(name_len), &mut payload).expect("serialize schema name");
        serialize_to_buffer(&vec!["x".to_owned()], &mut payload).expect("serialize schema fields");
        serialize_to_buffer(&(StateValueNodeV1::LEAF_TAG as u8), &mut payload)
            .expect("serialize Leaf tag");
        serialize_to_buffer(&(StateValueKindV1::Bool.tag() as u8), &mut payload)
            .expect("serialize leaf kind");
        payload
    }
    fn schema_name_len_at_payload_limit() -> usize {
        let payload_limit =
            state_value_payload_limit::<StateValueSchemaV1>(MAX_STATE_VALUE_SCHEMA_BYTES)
                .expect("schema payload limit");
        let mut accepted = 1usize;
        let mut rejected = payload_limit
            .checked_add(1)
            .expect("schema payload limit increment");
        while accepted < rejected {
            let candidate = accepted + (rejected - accepted) / 2;
            if encode_schema_payload_without_limit(candidate).len() <= payload_limit {
                accepted = candidate + 1;
            } else {
                rejected = candidate;
            }
        }
        let name_len = accepted - 1;
        assert_eq!(
            encode_schema_payload_without_limit(name_len).len(),
            payload_limit,
            "the canonical KSV1 string encoding must reach the exact frame boundary"
        );
        name_len
    }
    fn encode_record_payload_without_limit(pointer_len: usize) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&STATE_VALUE_RECORD_PAYLOAD_MAGIC_V1);
        payload.extend_from_slice(&[0x5a; 32]);
        payload.extend_from_slice(&1_u16.to_le_bytes());
        payload.push(StateValueAtomV1::POINTER_TAG as u8);
        payload.extend_from_slice(
            &u32::try_from(pointer_len)
                .expect("pointer length fits KRV1")
                .to_le_bytes(),
        );
        payload.resize(payload.len() + pointer_len, 0xa5);
        payload
    }
    fn wide_struct_schema(field_count: usize) -> StateValueSchemaV1 {
        let mut nodes = vec![StateValueNodeV1::Struct {
            name: "Wide".into(),
            fields: (0..field_count)
                .map(|index| format!("field_{index}"))
                .collect(),
        }];
        nodes.extend((0..field_count).map(|_| StateValueNodeV1::Leaf(StateValueKindV1::Bool)));
        StateValueSchemaV1 { nodes }
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
    fn exact_record_decoder_rejects_alternate_layouts_and_logical_tails() {
        let record = StateValueRecordV1 {
            schema_hash: [0x5a; 32],
            atoms: vec![StateValueAtomV1::Bool(true)],
        };
        let canonical = norito::encode_canonical(&record).expect("encode canonical KRV1 frame");
        assert_eq!(
            canonical[norito::core::Header::SIZE - 1],
            STATE_VALUE_RECORD_FRAME_FLAGS_V1,
            "KRV1's fixed-width byte wrapper must not advertise unused layout flags"
        );
        {
            let ambient_flags = norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(ambient_flags);
            let ambient_payload = b"ambient payload";
            let _ambient_payload = norito::core::PayloadCtxGuard::enter(ambient_payload);
            let payload_context_before = norito::core::payload_ctx();
            assert_eq!(
                decode_canonical_state_value_record_v1(&canonical)
                    .expect("decode exact canonical KRV1 frame"),
                record
            );
            assert_eq!(norito::core::get_decode_flags(), ambient_flags);
            assert_eq!(norito::core::payload_ctx(), payload_context_before);
        }
        let mut padded = canonical.clone();
        padded.insert(norito::core::Header::SIZE, 0);
        assert!(matches!(
            decode_canonical_state_value_record_v1(&padded),
            Err(NoritoError::LengthMismatch)
        ));
        let mut corrupt = canonical.clone();
        *corrupt.last_mut().expect("canonical frame has payload") ^= 0x80;
        {
            let ambient_flags = norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(ambient_flags);
            let ambient_payload = b"ambient checksum payload";
            let _ambient_payload = norito::core::PayloadCtxGuard::enter(ambient_payload);
            let payload_context_before = norito::core::payload_ctx();
            assert!(matches!(
                decode_canonical_state_value_record_v1(&corrupt),
                Err(NoritoError::ChecksumMismatch)
            ));
            assert_eq!(norito::core::get_decode_flags(), ambient_flags);
            assert_eq!(norito::core::payload_ctx(), payload_context_before);
        }
        let mut wrong_schema = canonical.clone();
        wrong_schema[6] ^= 0x80;
        assert!(matches!(
            decode_canonical_state_value_record_v1(&wrong_schema),
            Err(NoritoError::SchemaMismatch)
        ));
        let compressible_record = StateValueRecordV1 {
            schema_hash: [0x5a; 32],
            atoms: vec![StateValueAtomV1::Pointer(vec![0; 4 * 1024])],
        };
        let compressed = norito::to_compressed_bytes(
            &compressible_record,
            Some(norito::CompressionConfig::default()),
        )
        .expect("encode compressed KRV1 frame");
        assert!(matches!(
            decode_canonical_state_value_record_v1(&compressed),
            Err(NoritoError::NonCanonicalEncoding)
        ));
        let alternate_flags =
            STATE_VALUE_RECORD_FRAME_FLAGS_V1 | norito::core::header_flags::COMPACT_LEN;
        let alternate_payload = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            let payload =
                encode_state_value_record_payload(&record).expect("encode alternate KRV1 payload");
            let mut wrapped = Vec::new();
            serialize_to_buffer(&payload, &mut wrapped).expect("wrap alternate KRV1 payload");
            wrapped
        };
        let alternate = norito::core::frame_bare_with_header_flags::<StateValueRecordV1>(
            &alternate_payload,
            alternate_flags,
        )
        .expect("frame alternate KRV1 payload");
        assert!(matches!(
            decode_canonical_state_value_record_v1(&alternate),
            Err(NoritoError::NonCanonicalEncoding)
        ));
        let payload =
            encode_state_value_record_payload(&record).expect("encode canonical KRV1 payload");
        let mut wrapped = Vec::new();
        {
            let _canonical =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            serialize_to_buffer(&payload, &mut wrapped).expect("wrap canonical KRV1 payload");
        }
        wrapped.push(0);
        let tailed = norito::core::frame_bare_with_header_flags::<StateValueRecordV1>(
            &wrapped,
            STATE_VALUE_RECORD_FRAME_FLAGS_V1,
        )
        .expect("frame tailed KRV1 payload");
        assert!(matches!(
            decode_canonical_state_value_record_v1(&tailed),
            Err(NoritoError::LengthMismatch)
        ));
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
    #[test]
    fn recursive_list_schema_boundary_is_stack_safe() {
        let worker = std::thread::Builder::new()
            .name("state-value-schema-boundary".into())
            .stack_size(128 * 1024)
            .spawn(|| {
                let accepted = nested_list_schema(MAX_STATE_VALUE_NODES - 1);
                let rejected = nested_list_schema(MAX_STATE_VALUE_NODES);
                let accepted_valid = accepted.validate();
                let accepted_words = accepted.word_count();
                let rejected_valid = rejected.validate();
                (
                    accepted,
                    rejected,
                    accepted_valid,
                    accepted_words,
                    rejected_valid,
                )
            })
            .expect("spawn small-stack schema validator");
        let (accepted, rejected, accepted_valid, accepted_words, rejected_valid) =
            worker.join().expect("small-stack schema validator");
        assert!(accepted_valid, "255 Lists plus one leaf is 256 nodes");
        assert_eq!(accepted_words, Some(1));
        assert!(
            !rejected_valid,
            "256 Lists plus one leaf exceeds the shared node and depth limit"
        );
        drop_schema_iteratively(accepted);
        drop_schema_iteratively(rejected);
    }
    #[test]
    fn recursive_list_schema_boundary_roundtrips_canonically_on_a_small_stack() {
        let worker = std::thread::Builder::new()
            .name("state-value-schema-wire-boundary".into())
            .stack_size(128 * 1024)
            .spawn(|| -> Result<_, String> {
                let schema = nested_list_schema(MAX_STATE_VALUE_NODES - 1);
                let encoded =
                    norito::encode_canonical(&schema).map_err(|error| error.to_string())?;
                let decoded = norito::decode_canonical::<StateValueSchemaV1>(&encoded)
                    .map_err(|error| error.to_string())?;
                let reencoded =
                    norito::encode_canonical(&decoded).map_err(|error| error.to_string())?;
                let decoded_valid = decoded.validate();
                drop(schema);
                drop(decoded);
                Ok((encoded, reencoded, decoded_valid))
            })
            .expect("spawn small-stack schema wire roundtrip");
        let (encoded, reencoded, decoded_valid) = worker
            .join()
            .expect("small-stack schema wire roundtrip")
            .expect("boundary schema must roundtrip canonically");
        assert!(decoded_valid);
        assert_eq!(encoded, reencoded);
        let rejected = nested_list_schema(MAX_STATE_VALUE_NODES);
        assert!(
            norito::encode_canonical(&rejected).is_err(),
            "the 257-node schema must not be encodable"
        );
        drop_schema_iteratively(rejected);
    }
    #[test]
    fn schema_byte_limit_covers_the_complete_canonical_frame() {
        let payload_limit =
            state_value_payload_limit::<StateValueSchemaV1>(MAX_STATE_VALUE_SCHEMA_BYTES)
                .expect("schema payload limit");
        let boundary_name_len = schema_name_len_at_payload_limit();
        let schema = schema_with_name_len(boundary_name_len);
        let payload =
            encode_state_value_schema_payload(&schema).expect("encode boundary KSV1 payload");
        assert_eq!(payload.len(), payload_limit);
        assert_eq!(
            state_value_complete_frame_len::<StateValueSchemaV1>(payload.len())
                .expect("schema frame length"),
            MAX_STATE_VALUE_SCHEMA_BYTES
        );
        let frame = norito::encode_canonical(&schema).expect("encode boundary schema frame");
        assert_eq!(frame.len(), MAX_STATE_VALUE_SCHEMA_BYTES);
        let decoded = norito::decode_canonical::<StateValueSchemaV1>(&frame)
            .expect("decode boundary schema frame");
        assert_eq!(decoded, schema);
        let oversized_name_len = boundary_name_len + 1;
        let oversized = schema_with_name_len(oversized_name_len);
        assert!(encode_state_value_schema_payload(&oversized).is_err());
        assert!(norito::encode_canonical(&oversized).is_err());
        let oversized_payload = encode_schema_payload_without_limit(oversized_name_len);
        assert_eq!(
            state_value_complete_frame_len::<StateValueSchemaV1>(oversized_payload.len())
                .expect("oversized schema frame length"),
            MAX_STATE_VALUE_SCHEMA_BYTES + 1
        );
        assert!(decode_state_value_schema_payload(&oversized_payload).is_err());
        let mut wrapped = Vec::new();
        serialize_to_buffer(&oversized_payload, &mut wrapped).expect("wrap oversized KSV1 payload");
        assert!(StateValueSchemaV1::decode_from_slice(&wrapped).is_err());
        let oversized_frame = norito::core::frame_bare_with_header_flags::<StateValueSchemaV1>(
            &wrapped,
            frame[norito::core::Header::SIZE - 1],
        )
        .expect("frame oversized KSV1 payload");
        assert_eq!(oversized_frame.len(), MAX_STATE_VALUE_SCHEMA_BYTES + 1);
        assert!(norito::decode_from_bytes::<StateValueSchemaV1>(&oversized_frame).is_err());
    }
    #[test]
    fn flat_schema_payload_rejects_noncanonical_boundaries() {
        let schema = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Quantity),
            ],
        };
        let encoded =
            encode_state_value_schema_payload(&schema).expect("encode flat schema payload");
        assert_eq!(
            encoded,
            [
                STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1.as_slice(),
                &[2, 0],
                &[StateValueNodeV1::OPTION_TAG as u8],
                &[StateValueNodeV1::LEAF_TAG as u8],
                &[StateValueKindV1::Quantity.tag() as u8],
            ]
            .concat(),
            "KSV1 Option<Quantity> golden payload"
        );
        let decoded =
            decode_state_value_schema_payload(&encoded).expect("decode flat schema payload");
        assert_eq!(
            encode_state_value_schema_payload(&decoded).expect("re-encode flat schema payload"),
            encoded
        );
        let mut malformed = encoded.clone();
        malformed[0] ^= 0xff;
        assert!(decode_state_value_schema_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1.len()] = 0;
        malformed[STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1.len() + 1] = 0;
        assert!(decode_state_value_schema_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1.len() + 2] = u8::MAX;
        assert!(decode_state_value_schema_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed.push(0);
        assert!(decode_state_value_schema_payload(&malformed).is_err());
        for end in 0..encoded.len() {
            assert!(
                decode_state_value_schema_payload(&encoded[..end]).is_err(),
                "truncation at byte {end} must reject"
            );
        }
        let mut forged_fields = Vec::new();
        forged_fields.extend_from_slice(&STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1);
        forged_fields.extend_from_slice(&1_u16.to_le_bytes());
        forged_fields.push(StateValueNodeV1::STRUCT_TAG as u8);
        serialize_to_buffer(&"S".to_owned(), &mut forged_fields)
            .expect("serialize forged Struct name");
        forged_fields.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(
            decode_state_value_schema_payload(&forged_fields).is_err(),
            "a forged field count must reject before Vec<String> preallocation"
        );
    }
    #[test]
    fn flat_schema_payload_pins_asymmetric_result_and_list_layouts() {
        let result = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Result,
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
                StateValueNodeV1::Leaf(StateValueKindV1::Bool),
            ],
        };
        assert_eq!(
            encode_state_value_schema_payload(&result).expect("encode Result<Int, Bool>"),
            [
                STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1.as_slice(),
                &[3, 0],
                &[
                    StateValueNodeV1::RESULT_TAG as u8,
                    StateValueNodeV1::LEAF_TAG as u8,
                    StateValueKindV1::Int.tag() as u8,
                    StateValueNodeV1::LEAF_TAG as u8,
                    StateValueKindV1::Bool.tag() as u8,
                ],
            ]
            .concat(),
        );
        let list = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(StateValueSchemaV1 {
                    nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Bool)],
                }),
                capacity: 7,
            }],
        };
        assert_eq!(
            encode_state_value_schema_payload(&list).expect("encode List<Bool, 7>"),
            [
                STATE_VALUE_SCHEMA_PAYLOAD_MAGIC_V1.as_slice(),
                &[2, 0],
                &[
                    StateValueNodeV1::LIST_TAG as u8,
                    7,
                    StateValueNodeV1::LEAF_TAG as u8,
                    StateValueKindV1::Bool.tag() as u8,
                ],
            ]
            .concat(),
        );
    }
    #[test]
    fn record_byte_limit_covers_the_complete_canonical_frame() {
        let payload_limit =
            state_value_payload_limit::<StateValueRecordV1>(MAX_STATE_VALUE_RECORD_BYTES)
                .expect("record payload limit");
        let fixed_payload_len = encode_record_payload_without_limit(0).len();
        let pointer_len = payload_limit
            .checked_sub(fixed_payload_len)
            .expect("KRV1 fixed payload fits the record limit");
        let record = StateValueRecordV1 {
            schema_hash: [0x5a; 32],
            atoms: vec![StateValueAtomV1::Pointer(vec![0xa5; pointer_len])],
        };
        let payload =
            encode_state_value_record_payload(&record).expect("encode boundary KRV1 payload");
        assert_eq!(payload.len(), payload_limit);
        assert_eq!(
            state_value_complete_frame_len::<StateValueRecordV1>(payload.len())
                .expect("record frame length"),
            MAX_STATE_VALUE_RECORD_BYTES
        );
        let frame = norito::encode_canonical(&record).expect("encode boundary record frame");
        assert_eq!(frame.len(), MAX_STATE_VALUE_RECORD_BYTES);
        let decoded = norito::decode_canonical::<StateValueRecordV1>(&frame)
            .expect("decode boundary record frame");
        assert_eq!(decoded, record);
        let oversized_pointer_len = pointer_len + 1;
        let oversized = StateValueRecordV1 {
            schema_hash: [0x5a; 32],
            atoms: vec![StateValueAtomV1::Pointer(vec![0xa5; oversized_pointer_len])],
        };
        assert!(encode_state_value_record_payload(&oversized).is_err());
        assert!(norito::encode_canonical(&oversized).is_err());
        let oversized_payload = encode_record_payload_without_limit(oversized_pointer_len);
        assert_eq!(
            state_value_complete_frame_len::<StateValueRecordV1>(oversized_payload.len())
                .expect("oversized record frame length"),
            MAX_STATE_VALUE_RECORD_BYTES + 1
        );
        assert!(decode_state_value_record_payload(&oversized_payload).is_err());
        let mut wrapped = Vec::new();
        serialize_to_buffer(&oversized_payload, &mut wrapped).expect("wrap oversized KRV1 payload");
        assert!(StateValueRecordV1::decode_from_slice(&wrapped).is_err());
        let oversized_frame = norito::core::frame_bare_with_header_flags::<StateValueRecordV1>(
            &wrapped,
            frame[norito::core::Header::SIZE - 1],
        )
        .expect("frame oversized KRV1 payload");
        assert_eq!(oversized_frame.len(), MAX_STATE_VALUE_RECORD_BYTES + 1);
        assert!(norito::decode_from_bytes::<StateValueRecordV1>(&oversized_frame).is_err());
    }
    #[test]
    fn flat_record_payload_has_one_exact_all_variants_golden() {
        let record = StateValueRecordV1 {
            schema_hash: [0xab; 32],
            atoms: vec![
                StateValueAtomV1::Tag(true),
                StateValueAtomV1::Bool(false),
                StateValueAtomV1::Pointer(vec![0xaa, 0xbb]),
                StateValueAtomV1::List(vec![
                    vec![StateValueAtomV1::Bool(true)],
                    vec![
                        StateValueAtomV1::Tag(false),
                        StateValueAtomV1::Pointer(Vec::new()),
                    ],
                ]),
            ],
        };
        let encoded = encode_state_value_record_payload(&record).expect("encode exact KRV1 golden");
        let expected = [
            STATE_VALUE_RECORD_PAYLOAD_MAGIC_V1.as_slice(),
            &[0xab; 32],
            &[4, 0],
            &[StateValueAtomV1::TAG_TAG as u8, 1],
            &[StateValueAtomV1::BOOL_TAG as u8, 0],
            &[StateValueAtomV1::POINTER_TAG as u8, 2, 0, 0, 0, 0xaa, 0xbb],
            &[StateValueAtomV1::LIST_TAG as u8, 2],
            &[1, 0, StateValueAtomV1::BOOL_TAG as u8, 1],
            &[
                2,
                0,
                StateValueAtomV1::TAG_TAG as u8,
                0,
                StateValueAtomV1::POINTER_TAG as u8,
                0,
                0,
                0,
                0,
            ],
        ]
        .concat();
        assert_eq!(encoded, expected);
        let decoded =
            decode_state_value_record_payload(&encoded).expect("decode exact KRV1 golden");
        assert_eq!(
            encode_state_value_record_payload(&decoded).expect("re-encode exact KRV1 golden"),
            encoded
        );
    }
    #[test]
    fn flat_record_decoder_precharges_all_owned_allocations() {
        let record = StateValueRecordV1 {
            schema_hash: [0x5a; 32],
            atoms: vec![
                StateValueAtomV1::Pointer(vec![0xa5; 32]),
                StateValueAtomV1::List(vec![
                    vec![StateValueAtomV1::Bool(true)],
                    vec![StateValueAtomV1::Pointer(vec![0x5a; 16])],
                ]),
            ],
        };
        let encoded =
            encode_state_value_record_payload(&record).expect("encode budgeted KRV1 payload");
        let no_allocation =
            norito::DecodeLimits::new(usize::MAX, encoded.len(), usize::MAX, 0, usize::MAX);
        assert!(matches!(
            norito::with_decode_limits(no_allocation, || {
                decode_state_value_record_payload(&encoded)
            }),
            Err(NoritoError::TotalAllocationExceeded { .. })
        ));
        let sufficient_allocation = encoded
            .len()
            .checked_mul(64)
            .and_then(|bytes| bytes.checked_add(64 * 1024))
            .expect("payload-derived allocation limit fits usize");
        let sufficient = norito::DecodeLimits::new(
            usize::MAX,
            encoded.len(),
            usize::MAX,
            sufficient_allocation,
            usize::MAX,
        );
        assert_eq!(
            norito::with_decode_limits(sufficient, || {
                decode_state_value_record_payload(&encoded)
            })
            .expect("sufficient outer allocation limit accepts KRV1 payload"),
            record
        );
    }
    #[test]
    fn flat_record_payload_rejects_every_noncanonical_boundary() {
        let record = StateValueRecordV1 {
            schema_hash: [0xab; 32],
            atoms: vec![
                StateValueAtomV1::Tag(true),
                StateValueAtomV1::Bool(false),
                StateValueAtomV1::Pointer(vec![0xaa, 0xbb]),
                StateValueAtomV1::List(vec![
                    vec![StateValueAtomV1::Bool(true)],
                    vec![
                        StateValueAtomV1::Tag(false),
                        StateValueAtomV1::Pointer(Vec::new()),
                    ],
                ]),
            ],
        };
        let encoded = encode_state_value_record_payload(&record).expect("encode KRV1");
        for end in 0..encoded.len() {
            assert!(
                decode_state_value_record_payload(&encoded[..end]).is_err(),
                "KRV1 truncation at byte {end} must reject"
            );
        }
        let mut malformed = encoded.clone();
        malformed[0] ^= 0xff;
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[36..38].copy_from_slice(&0_u16.to_le_bytes());
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[36..38].copy_from_slice(&257_u16.to_le_bytes());
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[38] = u8::MAX;
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[39] = 2;
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[41] = 2;
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[43..47].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[50] = MAX_STATE_VALUE_LIST_CAPACITY_V1 + 1;
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed[51..53].copy_from_slice(&0_u16.to_le_bytes());
        assert!(decode_state_value_record_payload(&malformed).is_err());
        let mut malformed = encoded.clone();
        malformed.push(0);
        assert!(decode_state_value_record_payload(&malformed).is_err());
    }
    #[test]
    fn flat_record_encoder_rejects_invalid_counts() {
        let empty = StateValueRecordV1 {
            schema_hash: [0; 32],
            atoms: Vec::new(),
        };
        assert!(encode_state_value_record_payload(&empty).is_err());
        let wide = StateValueRecordV1 {
            schema_hash: [0; 32],
            atoms: (0..=MAX_STATE_VALUE_WORDS)
                .map(|_| StateValueAtomV1::Bool(false))
                .collect(),
        };
        assert!(encode_state_value_record_payload(&wide).is_err());
        let list = StateValueRecordV1 {
            schema_hash: [0; 32],
            atoms: vec![StateValueAtomV1::List(
                (0..=MAX_STATE_VALUE_LIST_CAPACITY_V1)
                    .map(|_| vec![StateValueAtomV1::Bool(false)])
                    .collect(),
            )],
        };
        assert!(encode_state_value_record_payload(&list).is_err());
    }
    #[test]
    fn recursive_list_record_boundary_is_canonical_and_stack_safe() {
        let worker = std::thread::Builder::new()
            .name("state-value-record-wire-boundary".into())
            .stack_size(128 * 1024)
            .spawn(|| -> Result<(), String> {
                let record = StateValueRecordV1 {
                    schema_hash: [0x5a; 32],
                    atoms: nested_list_atoms(MAX_STATE_VALUE_NODES - 1),
                };
                let encoded =
                    norito::encode_canonical(&record).map_err(|error| error.to_string())?;
                let decoded = norito::decode_canonical::<StateValueRecordV1>(&encoded)
                    .map_err(|error| error.to_string())?;
                let reencoded =
                    norito::encode_canonical(&decoded).map_err(|error| error.to_string())?;
                if encoded != reencoded {
                    return Err("canonical KRV1 re-encode changed bytes".to_owned());
                }
                let mut trailing = encode_state_value_record_payload(&record)
                    .map_err(|error| error.to_string())?;
                trailing.push(0);
                if decode_state_value_record_payload(&trailing).is_ok() {
                    return Err("deep KRV1 trailing byte was accepted".to_owned());
                }
                drop(decoded);
                drop(record);
                let rejected = StateValueRecordV1 {
                    schema_hash: [0x5a; 32],
                    atoms: nested_list_atoms(MAX_STATE_VALUE_NODES),
                };
                if norito::encode_canonical(&rejected).is_ok() {
                    return Err("256 nested KRV1 Lists were accepted".to_owned());
                }
                drop(rejected);
                let empty_chain = StateValueRecordV1 {
                    schema_hash: [0x5a; 32],
                    atoms: (1..MAX_STATE_VALUE_NODES)
                        .fold(vec![StateValueAtomV1::List(Vec::new())], |item, _| {
                            vec![StateValueAtomV1::List(vec![item])]
                        }),
                };
                if encode_state_value_record_payload(&empty_chain).is_ok() {
                    return Err(
                        "256 nested KRV1 Lists ending in an empty List were accepted".to_owned(),
                    );
                }
                drop(empty_chain);
                Ok(())
            })
            .expect("spawn small-stack record wire test");
        worker
            .join()
            .expect("small-stack record wire test")
            .expect("record boundary must be stack safe");
    }
    #[test]
    fn boundary_walkers_are_stack_safe_for_flat_sums_and_recursive_lists() {
        let worker = std::thread::Builder::new()
            .name("state-value-walker-boundary".into())
            .stack_size(128 * 1024)
            .spawn(|| {
                let options = nested_option_schema(MAX_STATE_VALUE_NODES - 1);
                let active_option_atoms = (0..MAX_STATE_VALUE_NODES - 1)
                    .map(|_| StateValueAtomV1::Tag(true))
                    .chain([StateValueAtomV1::Bool(true)])
                    .collect::<Vec<_>>();
                let inactive_option_atoms = vec![StateValueAtomV1::Tag(false)];
                let option_results = (
                    options.word_kinds(),
                    options.word_kinds_for_atoms(&active_option_atoms),
                    options.validate_atoms(&active_option_atoms),
                    options.validate_atoms(&inactive_option_atoms),
                );
                let lists = nested_list_schema(MAX_STATE_VALUE_NODES - 1);
                let list_atoms = nested_list_atoms(MAX_STATE_VALUE_NODES - 1);
                let list_results = (
                    lists.word_kinds(),
                    lists.word_kinds_for_atoms(&list_atoms),
                    lists.validate_atoms(&list_atoms),
                );
                (
                    options,
                    active_option_atoms,
                    lists,
                    list_atoms,
                    option_results,
                    list_results,
                )
            })
            .expect("spawn small-stack state-value walker");
        let (options, active_option_atoms, lists, list_atoms, option_results, list_results) =
            worker.join().expect("small-stack state-value walker");
        assert_eq!(
            option_results,
            (
                Some(vec![StateValueWordKindV1::Sum]),
                Some(vec![StateValueWordKindV1::Sum]),
                true,
                true,
            )
        );
        assert_eq!(
            list_results,
            (
                Some(vec![StateValueWordKindV1::List]),
                Some(vec![StateValueWordKindV1::List]),
                true,
            )
        );
        drop_schema_iteratively(options);
        drop(active_option_atoms);
        drop_schema_iteratively(lists);
        drop_atoms_iteratively(list_atoms);
    }
    #[test]
    fn recursive_list_elements_share_the_exact_node_budget() {
        let accepted = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(wide_struct_schema(MAX_STATE_VALUE_NODES - 2)),
                capacity: 64,
            }],
        };
        assert!(
            accepted.validate(),
            "List + struct + 254 leaves is exactly 256 nodes"
        );
        assert_eq!(accepted.word_count(), Some(1));
        let rejected = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(wide_struct_schema(MAX_STATE_VALUE_NODES - 1)),
                capacity: 64,
            }],
        };
        assert!(
            !rejected.validate(),
            "List + struct + 255 leaves is 257 nodes"
        );
    }
    #[test]
    fn malformed_recursive_list_element_schemas_reject() {
        let invalid_elements = [
            StateValueSchemaV1 { nodes: Vec::new() },
            StateValueSchemaV1 {
                nodes: vec![
                    StateValueNodeV1::Leaf(StateValueKindV1::Bool),
                    StateValueNodeV1::Leaf(StateValueKindV1::Bool),
                ],
            },
            StateValueSchemaV1 {
                nodes: vec![
                    StateValueNodeV1::Option,
                    StateValueNodeV1::Leaf(StateValueKindV1::AssetHandle),
                ],
            },
        ];
        for element in invalid_elements {
            let schema = StateValueSchemaV1 {
                nodes: vec![StateValueNodeV1::List {
                    element: Box::new(element),
                    capacity: 1,
                }],
            };
            assert!(!schema.validate());
        }
        for capacity in [
            MIN_STATE_VALUE_LIST_CAPACITY_V1 - 1,
            MAX_STATE_VALUE_LIST_CAPACITY_V1 + 1,
        ] {
            let schema = StateValueSchemaV1 {
                nodes: vec![StateValueNodeV1::List {
                    element: Box::new(StateValueSchemaV1 {
                        nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Bool)],
                    }),
                    capacity,
                }],
            };
            assert!(!schema.validate());
        }
    }
    #[test]
    fn mixed_schema_preserves_preorder_and_active_only_words() {
        let schema = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Struct {
                    name: "Mixed".into(),
                    fields: vec!["pair".into(), "outcome".into(), "items".into()],
                },
                StateValueNodeV1::Tuple { arity: 2 },
                StateValueNodeV1::Leaf(StateValueKindV1::Bool),
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
                StateValueNodeV1::Result,
                StateValueNodeV1::Leaf(StateValueKindV1::Decimal),
                StateValueNodeV1::Struct {
                    name: "Failure".into(),
                    fields: vec!["remaining".into()],
                },
                StateValueNodeV1::Leaf(StateValueKindV1::Quantity),
                StateValueNodeV1::List {
                    element: Box::new(StateValueSchemaV1 {
                        nodes: vec![
                            StateValueNodeV1::Tuple { arity: 2 },
                            StateValueNodeV1::Leaf(StateValueKindV1::String),
                            StateValueNodeV1::Leaf(StateValueKindV1::Bool),
                        ],
                    }),
                    capacity: 2,
                },
            ],
        };
        let expected_words = vec![
            StateValueWordKindV1::Leaf(StateValueKindV1::Bool),
            StateValueWordKindV1::Sum,
            StateValueWordKindV1::Sum,
            StateValueWordKindV1::List,
        ];
        assert!(schema.validate());
        assert_eq!(schema.word_count(), Some(expected_words.len()));
        assert_eq!(schema.word_kinds(), Some(expected_words.clone()));
        let atoms = vec![
            StateValueAtomV1::Bool(true),
            StateValueAtomV1::Tag(true),
            StateValueAtomV1::Pointer(vec![1]),
            StateValueAtomV1::Tag(false),
            StateValueAtomV1::Pointer(vec![2]),
            StateValueAtomV1::List(vec![vec![
                StateValueAtomV1::Pointer(vec![3]),
                StateValueAtomV1::Bool(false),
            ]]),
        ];
        assert!(schema.validate_atoms(&atoms));
        assert_eq!(schema.word_kinds_for_atoms(&atoms), Some(expected_words));
    }
}
