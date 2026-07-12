//! Program metadata parser used when loading bytecode.
//!
//! Each compiled program begins with a small header describing the VM version,
//! enabled features and optional cycle limit.  This module defines a
//! [`ProgramMetadata`] structure and helpers for encoding and decoding this
//! header.
//!
//! The metadata header encodes the VM version, execution mode flags, optional
//! vector length and cycle limit.  It also reserves bits for hardware
//! transactional memory (HTM) support.

use std::io::Write;

use crate::error::VMError;
use iroha_data_model::smart_contract::manifest::{
    AccessSetHints, ContractErrorCodeDescriptor, EntryPointKind, EntrypointDescriptor,
    KotobaTranslationEntry, TriggerDescriptor,
};
use norito::{
    Decode, Encode,
    core::{Archived, DecodeFromSlice, Error as NoritoError, NoritoDeserialize, NoritoSerialize},
};

/// Domain separator for the canonical deployable contract artifact hash.
///
/// The hash deliberately covers the complete `.to` image, including the fixed
/// execution header. Contract debug information belongs in a sidecar and is
/// therefore not part of a deployable artifact.
pub const CONTRACT_CODE_HASH_DOMAIN: &[u8] = b"iroha:ivm:contract-artifact:v1\0";

/// Compute the canonical identity of a deployable IVM contract artifact.
///
/// Unlike the pre-release body-only hash, this binds every execution-relevant
/// header field as well as embedded interface metadata, literals, and code.
#[must_use]
pub fn contract_code_hash(artifact: &[u8]) -> iroha_crypto::Hash {
    iroha_crypto::Hash::new_from_chunks(&[CONTRACT_CODE_HASH_DOMAIN, artifact])
}

/// Maximum accepted logical vector length for admission.
pub const VECTOR_LENGTH_MAX: u8 = 64;

/// Magic prefix identifying IVM bytecode.
pub const MAGIC: &[u8; 4] = b"IVM\0";
/// Fixed IVM V1 header size: 17 bytes of execution metadata followed by the
/// authenticated 32-byte ABI descriptor hash.
pub const HEADER_SIZE: usize = 49;

/// Literal table section marker placed immediately after the metadata header
/// when compiled bytecode includes literal fixups.
pub const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";
/// Bit shift of the ABI-v1 literal kind in an `LTLB` table descriptor.
pub const LITERAL_KIND_SHIFT: u32 = 56;
/// Mask selecting the section-relative offset in an ABI-v1 literal descriptor.
pub const LITERAL_OFFSET_MASK: u64 = (1_u64 << LITERAL_KIND_SHIFT) - 1;

/// Canonical kinds carried by ABI-v1 indexed-literal table descriptors.
///
/// Each descriptor is one little-endian `u64`: the high byte is this kind and
/// the low 56 bits are an offset relative to the `LTLB` marker. Keeping the
/// kind in the authenticated table prevents `LDLIT` and `LDI64` from giving
/// the same bytes two incompatible interpretations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LiteralKindV1 {
    /// A complete pointer-ABI TLV envelope.
    PointerTlv = 0,
    /// Exactly eight little-endian bytes representing a signed `i64`.
    I64 = 1,
}

impl TryFrom<u8> for LiteralKindV1 {
    type Error = VMError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::PointerTlv),
            1 => Ok(Self::I64),
            _ => Err(VMError::InvalidMetadata),
        }
    }
}

/// Encode one canonical ABI-v1 `LTLB` table descriptor.
#[must_use]
pub const fn encode_literal_descriptor(kind: LiteralKindV1, relative_offset: u64) -> Option<u64> {
    if relative_offset > LITERAL_OFFSET_MASK {
        return None;
    }
    Some(((kind as u64) << LITERAL_KIND_SHIFT) | relative_offset)
}

/// Decode one canonical ABI-v1 `LTLB` table descriptor.
pub fn decode_literal_descriptor(raw: u64) -> Result<(LiteralKindV1, u64), VMError> {
    let kind = LiteralKindV1::try_from((raw >> LITERAL_KIND_SHIFT) as u8)?;
    Ok((kind, raw & LITERAL_OFFSET_MASK))
}
/// Embedded contract interface section marker used by self-describing contract artifacts.
pub const CONTRACT_INTERFACE_SECTION_MAGIC: [u8; 4] = *b"CNTR";
/// Compiler-owned local entrypoint that authenticates the terminal return target
/// of a Kotodama test-suite artifact.
///
/// Production contract admission rejects this reserved selector. Only the
/// crate-private Kotodama test preparation path accepts it.
pub const KOTO_TEST_RETURN_ENTRYPOINT: &str = "__koto_test_return";
/// Stable nominal Norito schema name for the first-release contract interface.
pub const CONTRACT_INTERFACE_SCHEMA_NAME_V1: &str = "iroha.kotodama.EmbeddedContractInterfaceV1";
/// Stable nominal Norito schema name for embedded durable-state type trees.
pub const EMBEDDED_STATE_TYPE_SCHEMA_NAME_V1: &str = "iroha.kotodama.EmbeddedStateTypeV1";
/// Embedded contract debug section marker used by self-describing contract artifacts.
pub const CONTRACT_DEBUG_SECTION_MAGIC: [u8; 4] = *b"DBG1";
/// Embedded contract execution-capability bit: zero-knowledge mode.
pub const CONTRACT_FEATURE_BIT_ZK: u64 = 1 << 0;
/// Embedded contract execution-capability bit: deterministic IVM vector mode.
pub const CONTRACT_FEATURE_BIT_VECTOR: u64 = 1 << 1;
/// Bitmask of all currently supported embedded execution-capability bits.
pub const CONTRACT_FEATURE_KNOWN_BITS: u64 = CONTRACT_FEATURE_BIT_ZK | CONTRACT_FEATURE_BIT_VECTOR;

const CONTRACT_INTERFACE_SECTION_HEADER_SIZE: usize = 8;
const CONTRACT_DEBUG_SECTION_HEADER_SIZE: usize = 8;

/// Artifact-local entrypoint metadata carried inside the required `CNTR` section.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedEntrypointDescriptor {
    pub name: String,
    pub kind: EntryPointKind,
    pub params: Vec<iroha_data_model::smart_contract::manifest::EntrypointParamDescriptor>,
    /// Exact schema used to encode a parameterized V1 public argument record.
    /// Zero-parameter entrypoints have no record and therefore no schema.
    pub argument_schema: Option<crate::entrypoint::EntrypointArgumentSchemaV1>,
    pub return_type: Option<String>,
    /// Exact recursive schema for a non-unit public return value.
    pub return_schema: Option<crate::entrypoint::EntrypointValueTypeV1>,
    pub permission: Option<String>,
    pub read_keys: Vec<String>,
    pub write_keys: Vec<String>,
    pub access_hints_complete: Option<bool>,
    pub access_hints_skipped: Vec<String>,
    pub triggers: Vec<TriggerDescriptor>,
    /// Entrypoint PC relative to the executable instruction stream (not the artifact start).
    pub entry_pc: u64,
}

impl EmbeddedEntrypointDescriptor {
    #[must_use]
    pub fn to_manifest_descriptor(&self) -> EntrypointDescriptor {
        EntrypointDescriptor {
            name: self.name.clone(),
            kind: self.kind,
            params: self.params.clone(),
            argument_schema: self.argument_schema.clone(),
            return_type: self.return_type.clone(),
            return_schema: self.return_schema.clone(),
            permission: self.permission.clone(),
            read_keys: self.read_keys.clone(),
            write_keys: self.write_keys.clone(),
            access_hints_complete: self.access_hints_complete,
            access_hints_skipped: self.access_hints_skipped.clone(),
            triggers: self.triggers.clone(),
        }
    }
}

/// Field descriptor for embedded durable state record types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedStateFieldDescriptor {
    pub name: String,
    pub ty: EmbeddedStateType,
}

/// Compact durable-state type schema embedded in contract artifacts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmbeddedStateType {
    Int,
    Decimal,
    Quantity,
    Bool,
    String,
    Bytes,
    DataSpaceId,
    AccountId,
    AssetDefinitionId,
    AssetId,
    NftId,
    DomainId,
    Name,
    Json,
    Tuple(Vec<EmbeddedStateType>),
    Struct {
        name: String,
        fields: Vec<EmbeddedStateFieldDescriptor>,
    },
    StateMap {
        key: Box<EmbeddedStateType>,
        value: Box<EmbeddedStateType>,
    },
    Option(Box<EmbeddedStateType>),
    Result {
        ok: Box<EmbeddedStateType>,
        err: Box<EmbeddedStateType>,
    },
    /// Bounded contiguous list whose capacity is part of the V1 schema.
    List {
        element: Box<EmbeddedStateType>,
        capacity: u8,
    },
}

impl EmbeddedStateType {
    /// Return the stable one-byte tag used by the custom CNTR type-tree codec.
    #[must_use]
    pub const fn wire_tag(&self) -> u8 {
        match self {
            Self::Int => EMBEDDED_STATE_TYPE_TAG_INT,
            Self::Decimal => EMBEDDED_STATE_TYPE_TAG_DECIMAL,
            Self::Quantity => EMBEDDED_STATE_TYPE_TAG_QUANTITY,
            Self::Bool => EMBEDDED_STATE_TYPE_TAG_BOOL,
            Self::String => EMBEDDED_STATE_TYPE_TAG_STRING,
            Self::Bytes => EMBEDDED_STATE_TYPE_TAG_BYTES,
            Self::DataSpaceId => EMBEDDED_STATE_TYPE_TAG_DATASPACE_ID,
            Self::AccountId => EMBEDDED_STATE_TYPE_TAG_ACCOUNT_ID,
            Self::AssetDefinitionId => EMBEDDED_STATE_TYPE_TAG_ASSET_DEFINITION_ID,
            Self::AssetId => EMBEDDED_STATE_TYPE_TAG_ASSET_ID,
            Self::NftId => EMBEDDED_STATE_TYPE_TAG_NFT_ID,
            Self::DomainId => EMBEDDED_STATE_TYPE_TAG_DOMAIN_ID,
            Self::Name => EMBEDDED_STATE_TYPE_TAG_NAME,
            Self::Json => EMBEDDED_STATE_TYPE_TAG_JSON,
            Self::Tuple(_) => EMBEDDED_STATE_TYPE_TAG_TUPLE,
            Self::Struct { .. } => EMBEDDED_STATE_TYPE_TAG_STRUCT,
            Self::StateMap { .. } => EMBEDDED_STATE_TYPE_TAG_STATE_MAP,
            Self::Option(_) => EMBEDDED_STATE_TYPE_TAG_OPTION,
            Self::Result { .. } => EMBEDDED_STATE_TYPE_TAG_RESULT,
            Self::List { .. } => EMBEDDED_STATE_TYPE_TAG_LIST,
        }
    }
}

const EMBEDDED_STATE_TYPE_TAG_INT: u8 = 0;
const EMBEDDED_STATE_TYPE_TAG_DECIMAL: u8 = 1;
const EMBEDDED_STATE_TYPE_TAG_QUANTITY: u8 = 2;
const EMBEDDED_STATE_TYPE_TAG_BOOL: u8 = 3;
const EMBEDDED_STATE_TYPE_TAG_STRING: u8 = 4;
const EMBEDDED_STATE_TYPE_TAG_BYTES: u8 = 5;
const EMBEDDED_STATE_TYPE_TAG_DATASPACE_ID: u8 = 6;
const EMBEDDED_STATE_TYPE_TAG_ACCOUNT_ID: u8 = 7;
const EMBEDDED_STATE_TYPE_TAG_ASSET_DEFINITION_ID: u8 = 8;
const EMBEDDED_STATE_TYPE_TAG_ASSET_ID: u8 = 9;
const EMBEDDED_STATE_TYPE_TAG_NFT_ID: u8 = 10;
const EMBEDDED_STATE_TYPE_TAG_DOMAIN_ID: u8 = 11;
const EMBEDDED_STATE_TYPE_TAG_NAME: u8 = 12;
const EMBEDDED_STATE_TYPE_TAG_JSON: u8 = 13;
const EMBEDDED_STATE_TYPE_TAG_TUPLE: u8 = 14;
const EMBEDDED_STATE_TYPE_TAG_STRUCT: u8 = 15;
const EMBEDDED_STATE_TYPE_TAG_STATE_MAP: u8 = 16;
const EMBEDDED_STATE_TYPE_TAG_OPTION: u8 = 17;
const EMBEDDED_STATE_TYPE_TAG_RESULT: u8 = 18;
const EMBEDDED_STATE_TYPE_TAG_LIST: u8 = 19;
/// Maximum recursive depth accepted by the first-release CNTR state-type codec.
pub const MAX_EMBEDDED_STATE_TYPE_DEPTH_V1: usize = 256;

fn embedded_state_type_depth_error(operation: &str) -> NoritoError {
    NoritoError::Message(format!(
        "embedded state type {operation} nesting exceeds {MAX_EMBEDDED_STATE_TYPE_DEPTH_V1} levels"
    ))
}

fn validate_embedded_state_type_iterative(
    root: &EmbeddedStateType,
    operation: &str,
) -> Result<(), NoritoError> {
    let mut pending = vec![(root, 1_usize, false)];
    while let Some((value, depth, inside_list)) = pending.pop() {
        if depth > MAX_EMBEDDED_STATE_TYPE_DEPTH_V1 {
            return Err(embedded_state_type_depth_error(operation));
        }
        let child_depth = depth
            .checked_add(1)
            .ok_or_else(|| embedded_state_type_depth_error(operation))?;
        match value {
            EmbeddedStateType::Tuple(items) => {
                pending.extend(
                    items
                        .iter()
                        .rev()
                        .map(|item| (item, child_depth, inside_list)),
                );
            }
            EmbeddedStateType::Struct { fields, .. } => {
                pending.extend(
                    fields
                        .iter()
                        .rev()
                        .map(|field| (&field.ty, child_depth, inside_list)),
                );
            }
            EmbeddedStateType::StateMap { key, value } => {
                if inside_list {
                    return Err(NoritoError::Message(
                        "embedded List elements cannot contain resource handles".to_owned(),
                    ));
                }
                pending.push((value, child_depth, false));
                pending.push((key, child_depth, false));
            }
            EmbeddedStateType::Option(value) => {
                pending.push((value, child_depth, inside_list));
            }
            EmbeddedStateType::Result { ok, err } => {
                pending.push((err, child_depth, inside_list));
                pending.push((ok, child_depth, inside_list));
            }
            EmbeddedStateType::List { element, capacity } => {
                if !(1..=64).contains(capacity) {
                    return Err(NoritoError::Message(format!(
                        "embedded List capacity must be in 1..=64, got {capacity}"
                    )));
                }
                pending.push((element, child_depth, true));
            }
            EmbeddedStateType::Int
            | EmbeddedStateType::Decimal
            | EmbeddedStateType::Quantity
            | EmbeddedStateType::Bool
            | EmbeddedStateType::String
            | EmbeddedStateType::Bytes
            | EmbeddedStateType::DataSpaceId
            | EmbeddedStateType::AccountId
            | EmbeddedStateType::AssetDefinitionId
            | EmbeddedStateType::AssetId
            | EmbeddedStateType::NftId
            | EmbeddedStateType::DomainId
            | EmbeddedStateType::Name
            | EmbeddedStateType::Json => {}
        }
    }
    Ok(())
}

fn expect_payload_consumed(
    consumed: usize,
    total: usize,
    context: &'static str,
) -> Result<(), NoritoError> {
    if consumed == total {
        return Ok(());
    }
    Err(NoritoError::Message(format!(
        "trailing bytes in {context} payload"
    )))
}

fn encode_embedded_state_owned_child(
    encoded: &[u8],
    writer: &mut Vec<u8>,
) -> Result<(), NoritoError> {
    let encoded_len = u64::try_from(encoded.len()).map_err(|_| NoritoError::LengthMismatch)?;
    let owned_payload_len = encoded
        .len()
        .checked_add(core::mem::size_of::<u64>())
        .and_then(|len| u64::try_from(len).ok())
        .ok_or(NoritoError::LengthMismatch)?;
    norito::core::write_len_header(writer, owned_payload_len)?;
    norito::core::write_seq_len(writer, encoded_len)?;
    writer.write_all(encoded)?;
    Ok(())
}

fn decode_embedded_state_owned_child(encoded: &[u8]) -> Result<(Vec<u8>, usize), NoritoError> {
    let (owned_payload_len, header_len) = norito::core::read_len_from_slice(encoded)?;
    let end = header_len
        .checked_add(owned_payload_len)
        .ok_or(NoritoError::LengthMismatch)?;
    let owned_payload = encoded
        .get(header_len..end)
        .ok_or(NoritoError::LengthMismatch)?;
    let (child, used) = <Vec<u8> as DecodeFromSlice>::decode_from_slice(owned_payload)?;
    expect_payload_consumed(used, owned_payload.len(), "EmbeddedStateType owned child")?;
    Ok((child, end))
}

fn encode_embedded_state_field_payload(
    value: &EmbeddedStateFieldDescriptor,
) -> Result<Vec<u8>, NoritoError> {
    let mut payload = Vec::new();
    value.name.serialize(&mut payload)?;
    value.ty.serialize(&mut payload)?;
    Ok(payload)
}

fn decode_embedded_state_field_payload(
    encoded: &[u8],
) -> Result<EmbeddedStateFieldDescriptor, NoritoError> {
    let (name, name_used) = <String as DecodeFromSlice>::decode_from_slice(encoded)?;
    let (ty, ty_used) =
        <EmbeddedStateType as DecodeFromSlice>::decode_from_slice(&encoded[name_used..])?;
    expect_payload_consumed(
        name_used + ty_used,
        encoded.len(),
        "EmbeddedStateFieldDescriptor",
    )?;
    Ok(EmbeddedStateFieldDescriptor { name, ty })
}

fn encode_embedded_state_type_payload(value: &EmbeddedStateType) -> Result<Vec<u8>, NoritoError> {
    enum Event<'a> {
        Enter(&'a EmbeddedStateType),
        Finish(&'a EmbeddedStateType),
    }

    validate_embedded_state_type_iterative(value, "encoding")?;
    let mut pending = vec![Event::Enter(value)];
    let mut encoded_values = Vec::<Vec<u8>>::new();
    while let Some(event) = pending.pop() {
        match event {
            Event::Enter(value) => {
                pending.push(Event::Finish(value));
                match value {
                    EmbeddedStateType::Tuple(items) => {
                        pending.extend(items.iter().rev().map(Event::Enter));
                    }
                    EmbeddedStateType::Struct { fields, .. } => {
                        pending.extend(fields.iter().rev().map(|field| Event::Enter(&field.ty)));
                    }
                    EmbeddedStateType::StateMap { key, value } => {
                        pending.push(Event::Enter(value));
                        pending.push(Event::Enter(key));
                    }
                    EmbeddedStateType::Option(value) => pending.push(Event::Enter(value)),
                    EmbeddedStateType::Result { ok, err } => {
                        pending.push(Event::Enter(err));
                        pending.push(Event::Enter(ok));
                    }
                    EmbeddedStateType::List { element, .. } => {
                        pending.push(Event::Enter(element));
                    }
                    EmbeddedStateType::Int
                    | EmbeddedStateType::Decimal
                    | EmbeddedStateType::Quantity
                    | EmbeddedStateType::Bool
                    | EmbeddedStateType::String
                    | EmbeddedStateType::Bytes
                    | EmbeddedStateType::DataSpaceId
                    | EmbeddedStateType::AccountId
                    | EmbeddedStateType::AssetDefinitionId
                    | EmbeddedStateType::AssetId
                    | EmbeddedStateType::NftId
                    | EmbeddedStateType::DomainId
                    | EmbeddedStateType::Name
                    | EmbeddedStateType::Json => {}
                }
            }
            Event::Finish(value) => {
                let child_count = match value {
                    EmbeddedStateType::Tuple(items) => items.len(),
                    EmbeddedStateType::Struct { fields, .. } => fields.len(),
                    EmbeddedStateType::StateMap { .. } | EmbeddedStateType::Result { .. } => 2,
                    EmbeddedStateType::Option(_) | EmbeddedStateType::List { .. } => 1,
                    EmbeddedStateType::Int
                    | EmbeddedStateType::Decimal
                    | EmbeddedStateType::Quantity
                    | EmbeddedStateType::Bool
                    | EmbeddedStateType::String
                    | EmbeddedStateType::Bytes
                    | EmbeddedStateType::DataSpaceId
                    | EmbeddedStateType::AccountId
                    | EmbeddedStateType::AssetDefinitionId
                    | EmbeddedStateType::AssetId
                    | EmbeddedStateType::NftId
                    | EmbeddedStateType::DomainId
                    | EmbeddedStateType::Name
                    | EmbeddedStateType::Json => 0,
                };
                let children_start =
                    encoded_values
                        .len()
                        .checked_sub(child_count)
                        .ok_or_else(|| {
                            NoritoError::Message(
                                "invalid iterative embedded state encoder state".to_owned(),
                            )
                        })?;
                let children = encoded_values.split_off(children_start);
                let mut child = children.into_iter();
                let mut payload = Vec::new();
                match value {
                    EmbeddedStateType::Int => {
                        EMBEDDED_STATE_TYPE_TAG_INT.serialize(&mut payload)?
                    }
                    EmbeddedStateType::Decimal => {
                        EMBEDDED_STATE_TYPE_TAG_DECIMAL.serialize(&mut payload)?
                    }
                    EmbeddedStateType::Quantity => {
                        EMBEDDED_STATE_TYPE_TAG_QUANTITY.serialize(&mut payload)?
                    }
                    EmbeddedStateType::Bool => {
                        EMBEDDED_STATE_TYPE_TAG_BOOL.serialize(&mut payload)?
                    }
                    EmbeddedStateType::String => {
                        EMBEDDED_STATE_TYPE_TAG_STRING.serialize(&mut payload)?
                    }
                    EmbeddedStateType::Bytes => {
                        EMBEDDED_STATE_TYPE_TAG_BYTES.serialize(&mut payload)?
                    }
                    EmbeddedStateType::DataSpaceId => {
                        EMBEDDED_STATE_TYPE_TAG_DATASPACE_ID.serialize(&mut payload)?
                    }
                    EmbeddedStateType::AccountId => {
                        EMBEDDED_STATE_TYPE_TAG_ACCOUNT_ID.serialize(&mut payload)?
                    }
                    EmbeddedStateType::AssetDefinitionId => {
                        EMBEDDED_STATE_TYPE_TAG_ASSET_DEFINITION_ID.serialize(&mut payload)?
                    }
                    EmbeddedStateType::AssetId => {
                        EMBEDDED_STATE_TYPE_TAG_ASSET_ID.serialize(&mut payload)?
                    }
                    EmbeddedStateType::NftId => {
                        EMBEDDED_STATE_TYPE_TAG_NFT_ID.serialize(&mut payload)?
                    }
                    EmbeddedStateType::DomainId => {
                        EMBEDDED_STATE_TYPE_TAG_DOMAIN_ID.serialize(&mut payload)?
                    }
                    EmbeddedStateType::Name => {
                        EMBEDDED_STATE_TYPE_TAG_NAME.serialize(&mut payload)?
                    }
                    EmbeddedStateType::Json => {
                        EMBEDDED_STATE_TYPE_TAG_JSON.serialize(&mut payload)?
                    }
                    EmbeddedStateType::Tuple(_) => {
                        EMBEDDED_STATE_TYPE_TAG_TUPLE.serialize(&mut payload)?;
                        child.by_ref().collect::<Vec<_>>().serialize(&mut payload)?;
                    }
                    EmbeddedStateType::Struct { name, fields } => {
                        EMBEDDED_STATE_TYPE_TAG_STRUCT.serialize(&mut payload)?;
                        name.serialize(&mut payload)?;
                        let mut encoded_fields = Vec::with_capacity(fields.len());
                        for field in fields {
                            let mut encoded_field = Vec::new();
                            field.name.serialize(&mut encoded_field)?;
                            child
                                .next()
                                .ok_or_else(|| {
                                    NoritoError::Message(
                                        "missing iterative embedded state field value".to_owned(),
                                    )
                                })?
                                .serialize(&mut encoded_field)?;
                            encoded_fields.push(encoded_field);
                        }
                        encoded_fields.serialize(&mut payload)?;
                    }
                    EmbeddedStateType::StateMap { .. } => {
                        EMBEDDED_STATE_TYPE_TAG_STATE_MAP.serialize(&mut payload)?;
                        let key = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state map key".to_owned(),
                            )
                        })?;
                        encode_embedded_state_owned_child(&key, &mut payload)?;
                        let value = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state map value".to_owned(),
                            )
                        })?;
                        encode_embedded_state_owned_child(&value, &mut payload)?;
                    }
                    EmbeddedStateType::Option(_) => {
                        EMBEDDED_STATE_TYPE_TAG_OPTION.serialize(&mut payload)?;
                        let value = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state option value".to_owned(),
                            )
                        })?;
                        encode_embedded_state_owned_child(&value, &mut payload)?;
                    }
                    EmbeddedStateType::Result { .. } => {
                        EMBEDDED_STATE_TYPE_TAG_RESULT.serialize(&mut payload)?;
                        let ok = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state result ok value".to_owned(),
                            )
                        })?;
                        encode_embedded_state_owned_child(&ok, &mut payload)?;
                        let err = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state result error value".to_owned(),
                            )
                        })?;
                        encode_embedded_state_owned_child(&err, &mut payload)?;
                    }
                    EmbeddedStateType::List { capacity, .. } => {
                        EMBEDDED_STATE_TYPE_TAG_LIST.serialize(&mut payload)?;
                        let element = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state list element".to_owned(),
                            )
                        })?;
                        encode_embedded_state_owned_child(&element, &mut payload)?;
                        capacity.serialize(&mut payload)?;
                    }
                }
                if child.next().is_some() {
                    return Err(NoritoError::Message(
                        "extra iterative embedded state encoder child".to_owned(),
                    ));
                }
                encoded_values.push(payload);
            }
        }
    }
    if encoded_values.len() != 1 {
        return Err(NoritoError::Message(
            "invalid iterative embedded state encoder result".to_owned(),
        ));
    }
    encoded_values.pop().ok_or(NoritoError::LengthMismatch)
}

fn decode_embedded_state_type_payload(encoded: &[u8]) -> Result<EmbeddedStateType, NoritoError> {
    enum Constructor {
        Tuple(usize),
        Struct {
            name: String,
            field_names: Vec<String>,
        },
        StateMap,
        Option,
        Result,
        List(u8),
    }

    impl Constructor {
        fn child_count(&self) -> usize {
            match self {
                Self::Tuple(count) => *count,
                Self::Struct { field_names, .. } => field_names.len(),
                Self::StateMap | Self::Result => 2,
                Self::Option | Self::List(_) => 1,
            }
        }
    }

    enum Event {
        Decode { encoded: Vec<u8>, depth: usize },
        Finish(Constructor),
    }

    struct DecodedValue {
        value: EmbeddedStateType,
        contains_resource_handle: bool,
    }

    let mut pending = vec![Event::Decode {
        encoded: encoded.to_vec(),
        depth: 1,
    }];
    let mut decoded_values = Vec::<DecodedValue>::new();
    while let Some(event) = pending.pop() {
        match event {
            Event::Decode { encoded, depth } => {
                if depth > MAX_EMBEDDED_STATE_TYPE_DEPTH_V1 {
                    return Err(embedded_state_type_depth_error("decoding"));
                }
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| embedded_state_type_depth_error("decoding"))?;
                let (tag, tag_used) = <u8 as DecodeFromSlice>::decode_from_slice(&encoded)?;
                let payload = &encoded[tag_used..];
                let (constructor, children, consumed) = match tag {
                    EMBEDDED_STATE_TYPE_TAG_INT
                    | EMBEDDED_STATE_TYPE_TAG_DECIMAL
                    | EMBEDDED_STATE_TYPE_TAG_QUANTITY
                    | EMBEDDED_STATE_TYPE_TAG_BOOL
                    | EMBEDDED_STATE_TYPE_TAG_STRING
                    | EMBEDDED_STATE_TYPE_TAG_BYTES
                    | EMBEDDED_STATE_TYPE_TAG_DATASPACE_ID
                    | EMBEDDED_STATE_TYPE_TAG_ACCOUNT_ID
                    | EMBEDDED_STATE_TYPE_TAG_ASSET_DEFINITION_ID
                    | EMBEDDED_STATE_TYPE_TAG_ASSET_ID
                    | EMBEDDED_STATE_TYPE_TAG_NFT_ID
                    | EMBEDDED_STATE_TYPE_TAG_DOMAIN_ID
                    | EMBEDDED_STATE_TYPE_TAG_NAME
                    | EMBEDDED_STATE_TYPE_TAG_JSON => {
                        expect_payload_consumed(0, payload.len(), "EmbeddedStateType")?;
                        let value = match tag {
                            EMBEDDED_STATE_TYPE_TAG_INT => EmbeddedStateType::Int,
                            EMBEDDED_STATE_TYPE_TAG_DECIMAL => EmbeddedStateType::Decimal,
                            EMBEDDED_STATE_TYPE_TAG_QUANTITY => EmbeddedStateType::Quantity,
                            EMBEDDED_STATE_TYPE_TAG_BOOL => EmbeddedStateType::Bool,
                            EMBEDDED_STATE_TYPE_TAG_STRING => EmbeddedStateType::String,
                            EMBEDDED_STATE_TYPE_TAG_BYTES => EmbeddedStateType::Bytes,
                            EMBEDDED_STATE_TYPE_TAG_DATASPACE_ID => EmbeddedStateType::DataSpaceId,
                            EMBEDDED_STATE_TYPE_TAG_ACCOUNT_ID => EmbeddedStateType::AccountId,
                            EMBEDDED_STATE_TYPE_TAG_ASSET_DEFINITION_ID => {
                                EmbeddedStateType::AssetDefinitionId
                            }
                            EMBEDDED_STATE_TYPE_TAG_ASSET_ID => EmbeddedStateType::AssetId,
                            EMBEDDED_STATE_TYPE_TAG_NFT_ID => EmbeddedStateType::NftId,
                            EMBEDDED_STATE_TYPE_TAG_DOMAIN_ID => EmbeddedStateType::DomainId,
                            EMBEDDED_STATE_TYPE_TAG_NAME => EmbeddedStateType::Name,
                            EMBEDDED_STATE_TYPE_TAG_JSON => EmbeddedStateType::Json,
                            other => {
                                return Err(NoritoError::invalid_tag(
                                    "EmbeddedStateType::try_deserialize",
                                    other,
                                ));
                            }
                        };
                        decoded_values.push(DecodedValue {
                            value,
                            contains_resource_handle: false,
                        });
                        continue;
                    }
                    EMBEDDED_STATE_TYPE_TAG_TUPLE => {
                        let (children, used) =
                            <Vec<Vec<u8>> as DecodeFromSlice>::decode_from_slice(payload)?;
                        (Constructor::Tuple(children.len()), children, used)
                    }
                    EMBEDDED_STATE_TYPE_TAG_STRUCT => {
                        let (name, name_used) =
                            <String as DecodeFromSlice>::decode_from_slice(payload)?;
                        let (encoded_fields, fields_used) =
                            <Vec<Vec<u8>> as DecodeFromSlice>::decode_from_slice(
                                &payload[name_used..],
                            )?;
                        let mut field_names = Vec::with_capacity(encoded_fields.len());
                        let mut children = Vec::with_capacity(encoded_fields.len());
                        for encoded_field in encoded_fields {
                            let (field_name, field_name_used) =
                                <String as DecodeFromSlice>::decode_from_slice(&encoded_field)?;
                            let (field_type, field_type_used) =
                                <Vec<u8> as DecodeFromSlice>::decode_from_slice(
                                    &encoded_field[field_name_used..],
                                )?;
                            expect_payload_consumed(
                                field_name_used + field_type_used,
                                encoded_field.len(),
                                "EmbeddedStateFieldDescriptor",
                            )?;
                            field_names.push(field_name);
                            children.push(field_type);
                        }
                        (
                            Constructor::Struct { name, field_names },
                            children,
                            name_used + fields_used,
                        )
                    }
                    EMBEDDED_STATE_TYPE_TAG_STATE_MAP => {
                        let (key, key_used) = decode_embedded_state_owned_child(payload)?;
                        let (value, value_used) =
                            decode_embedded_state_owned_child(&payload[key_used..])?;
                        (
                            Constructor::StateMap,
                            vec![key, value],
                            key_used + value_used,
                        )
                    }
                    EMBEDDED_STATE_TYPE_TAG_OPTION => {
                        let (value, value_used) = decode_embedded_state_owned_child(payload)?;
                        (Constructor::Option, vec![value], value_used)
                    }
                    EMBEDDED_STATE_TYPE_TAG_RESULT => {
                        let (ok, ok_used) = decode_embedded_state_owned_child(payload)?;
                        let (err, err_used) =
                            decode_embedded_state_owned_child(&payload[ok_used..])?;
                        (Constructor::Result, vec![ok, err], ok_used + err_used)
                    }
                    EMBEDDED_STATE_TYPE_TAG_LIST => {
                        let (element, element_used) = decode_embedded_state_owned_child(payload)?;
                        let (capacity, capacity_used) =
                            <u8 as DecodeFromSlice>::decode_from_slice(&payload[element_used..])?;
                        if !(1..=64).contains(&capacity) {
                            return Err(NoritoError::Message(format!(
                                "embedded List capacity must be in 1..=64, got {capacity}"
                            )));
                        }
                        (
                            Constructor::List(capacity),
                            vec![element],
                            element_used + capacity_used,
                        )
                    }
                    other => {
                        return Err(NoritoError::invalid_tag(
                            "EmbeddedStateType::try_deserialize",
                            other,
                        ));
                    }
                };
                expect_payload_consumed(consumed, payload.len(), "EmbeddedStateType")?;
                pending.push(Event::Finish(constructor));
                pending.extend(children.into_iter().rev().map(|encoded| Event::Decode {
                    encoded,
                    depth: child_depth,
                }));
            }
            Event::Finish(constructor) => {
                let child_count = constructor.child_count();
                let children_start =
                    decoded_values
                        .len()
                        .checked_sub(child_count)
                        .ok_or_else(|| {
                            NoritoError::Message(
                                "invalid iterative embedded state decoder state".to_owned(),
                            )
                        })?;
                let children = decoded_values.split_off(children_start);
                let contains_resource_handle =
                    children.iter().any(|child| child.contains_resource_handle);
                let value = match constructor {
                    Constructor::Tuple(_) => EmbeddedStateType::Tuple(
                        children.into_iter().map(|child| child.value).collect(),
                    ),
                    Constructor::Struct { name, field_names } => EmbeddedStateType::Struct {
                        name,
                        fields: field_names
                            .into_iter()
                            .zip(children)
                            .map(|(name, child)| EmbeddedStateFieldDescriptor {
                                name,
                                ty: child.value,
                            })
                            .collect(),
                    },
                    Constructor::StateMap => {
                        let mut child = children.into_iter();
                        let key = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state map key".to_owned(),
                            )
                        })?;
                        let value = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state map value".to_owned(),
                            )
                        })?;
                        EmbeddedStateType::StateMap {
                            key: Box::new(key.value),
                            value: Box::new(value.value),
                        }
                    }
                    Constructor::Option => {
                        let value = children.into_iter().next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state option value".to_owned(),
                            )
                        })?;
                        EmbeddedStateType::Option(Box::new(value.value))
                    }
                    Constructor::Result => {
                        let mut child = children.into_iter();
                        let ok = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state result ok value".to_owned(),
                            )
                        })?;
                        let err = child.next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state result error value".to_owned(),
                            )
                        })?;
                        EmbeddedStateType::Result {
                            ok: Box::new(ok.value),
                            err: Box::new(err.value),
                        }
                    }
                    Constructor::List(capacity) => {
                        if contains_resource_handle {
                            return Err(NoritoError::Message(
                                "embedded List elements cannot contain resource handles".to_owned(),
                            ));
                        }
                        let element = children.into_iter().next().ok_or_else(|| {
                            NoritoError::Message(
                                "missing iterative embedded state list element".to_owned(),
                            )
                        })?;
                        EmbeddedStateType::List {
                            element: Box::new(element.value),
                            capacity,
                        }
                    }
                };
                decoded_values.push(DecodedValue {
                    contains_resource_handle: matches!(value, EmbeddedStateType::StateMap { .. })
                        || contains_resource_handle,
                    value,
                });
            }
        }
    }
    if decoded_values.len() != 1 {
        return Err(NoritoError::Message(
            "invalid iterative embedded state decoder result".to_owned(),
        ));
    }
    decoded_values
        .pop()
        .map(|decoded| decoded.value)
        .ok_or(NoritoError::LengthMismatch)
}

impl NoritoSerialize for EmbeddedStateFieldDescriptor {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), NoritoError> {
        let encoded = encode_embedded_state_field_payload(self)?;
        encoded.serialize(&mut writer)
    }
}

impl<'a> NoritoDeserialize<'a> for EmbeddedStateFieldDescriptor {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("EmbeddedStateFieldDescriptor decode")
    }

    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, NoritoError> {
        let encoded = <Vec<u8> as NoritoDeserialize>::try_deserialize(archived.cast::<Vec<u8>>())?;
        decode_embedded_state_field_payload(&encoded)
    }
}

impl<'a> DecodeFromSlice<'a> for EmbeddedStateFieldDescriptor {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        let (encoded, used) = <Vec<u8> as DecodeFromSlice>::decode_from_slice(bytes)?;
        let value = decode_embedded_state_field_payload(&encoded)?;
        Ok((value, used))
    }
}

impl NoritoSerialize for EmbeddedStateType {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(EMBEDDED_STATE_TYPE_SCHEMA_NAME_V1)
    }

    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), NoritoError> {
        let encoded = encode_embedded_state_type_payload(self)?;
        encoded.serialize(&mut writer)
    }
}

impl<'a> NoritoDeserialize<'a> for EmbeddedStateType {
    fn schema_hash() -> [u8; 16] {
        norito::core::schema_hash_for_name(EMBEDDED_STATE_TYPE_SCHEMA_NAME_V1)
    }

    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("EmbeddedStateType decode")
    }

    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, NoritoError> {
        let encoded = <Vec<u8> as NoritoDeserialize>::try_deserialize(archived.cast::<Vec<u8>>())?;
        decode_embedded_state_type_payload(&encoded)
    }
}

impl<'a> DecodeFromSlice<'a> for EmbeddedStateType {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        let (encoded, used) = <Vec<u8> as DecodeFromSlice>::decode_from_slice(bytes)?;
        let value = decode_embedded_state_type_payload(&encoded)?;
        Ok((value, used))
    }
}

/// Seiyaku-level durable state declaration descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedStateDescriptor {
    pub name: String,
    pub ty: EmbeddedStateType,
}

/// Decoded payload of the required `CNTR` section carried by contract artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kotodama.EmbeddedContractInterfaceV1")]
pub struct EmbeddedContractInterfaceV1 {
    /// Canonical source-level seiyaku identity.
    pub seiyaku_name: String,
    pub compiler_fingerprint: String,
    /// Canonical hash of the exact ABI-v1 descriptor targeted by this artifact.
    ///
    /// The verifier compares this authenticated field with its local ABI
    /// descriptor before accepting the artifact, so an old artifact cannot be
    /// silently reinterpreted under changed syscall, pointer, gas, or state
    /// semantics while retaining `abi_version = 1`.
    pub abi_hash: [u8; 32],
    /// Compiler-derived ZK/VECTOR capability bits mirrored from the execution header.
    ///
    /// The complete `CNTR` section is covered by the artifact hash. These bits
    /// are unrelated to optional host hardware acceleration.
    pub features_bitmap: u64,
    pub access_set_hints: Option<AccessSetHints>,
    pub kotoba: Vec<KotobaTranslationEntry>,
    pub entrypoints: Vec<EmbeddedEntrypointDescriptor>,
    pub states: Vec<EmbeddedStateDescriptor>,
    /// Stable application error codes accepted by `require`.
    pub error_codes: Vec<ContractErrorCodeDescriptor>,
}

/// Exact source location emitted for hash-keyed compiler debug sidecars.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedSourceLocation {
    #[norito(default)]
    pub source_path: Option<String>,
    /// Stable source identity inside the compiler graph.
    pub source_id: u32,
    /// First included UTF-8 byte offset in the source.
    pub byte_start: u32,
    /// First excluded UTF-8 byte offset in the source.
    pub byte_end: u32,
    pub line: u32,
    pub column: u32,
}

/// One exact bytecode/source segment in compiler debug metadata.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedSourceMapEntryV1 {
    pub function_name: String,
    /// Function start PC relative to the executable instruction stream.
    pub pc_start: u64,
    /// Function end PC relative to the executable instruction stream.
    pub pc_end: u64,
    pub source: EmbeddedSourceLocation,
}

/// Function-level budget summary emitted inside the optional `DBG1` section.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedFunctionBudgetReportV1 {
    pub function_name: String,
    pub pc_start: u64,
    pub pc_end: u64,
    pub bytecode_bytes: u32,
    pub bytecode_words: u32,
    pub frame_bytes: u32,
    pub jump_span_words: u32,
    pub jump_range_risk: bool,
    pub source: Option<EmbeddedSourceLocation>,
}

/// Decoded payload of the optional `DBG1` section carried by contract artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedContractDebugInfoV1 {
    pub source_map: Vec<EmbeddedSourceMapEntryV1>,
    pub budget_report: Vec<EmbeddedFunctionBudgetReportV1>,
}

impl EmbeddedContractDebugInfoV1 {
    #[must_use]
    pub fn encode_section(&self) -> Vec<u8> {
        let payload =
            norito::to_bytes(self).expect("embedded contract debug encoding must succeed");
        let payload_len =
            u32::try_from(payload.len()).expect("embedded contract debug exceeds u32");
        let mut section = Vec::with_capacity(CONTRACT_DEBUG_SECTION_HEADER_SIZE + payload.len());
        section.extend_from_slice(&CONTRACT_DEBUG_SECTION_MAGIC);
        section.extend_from_slice(&payload_len.to_le_bytes());
        section.extend_from_slice(&payload);
        section
    }
}

impl EmbeddedContractInterfaceV1 {
    #[must_use]
    pub fn encode_section(&self) -> Vec<u8> {
        let payload =
            norito::to_bytes(self).expect("embedded contract interface encoding must succeed");
        let payload_len =
            u32::try_from(payload.len()).expect("embedded contract interface exceeds u32");
        let mut section =
            Vec::with_capacity(CONTRACT_INTERFACE_SECTION_HEADER_SIZE + payload.len());
        section.extend_from_slice(&CONTRACT_INTERFACE_SECTION_MAGIC);
        section.extend_from_slice(&payload_len.to_le_bytes());
        section.extend_from_slice(&payload);
        section
    }
}

/// Execution mode flags used in the metadata header.
pub mod mode {
    /// Zero-knowledge proof mode enabled.
    #[allow(dead_code)]
    pub const ZK: u8 = 0x01;
    /// Vector extension (SIMD/crypto ops) enabled.
    pub const VECTOR: u8 = 0x02;
    /// Hardware transactional memory enabled.
    #[allow(dead_code)]
    pub const HTM: u8 = 0x04;
}

#[derive(Clone, Debug)]
pub struct ProgramMetadata {
    pub version_major: u8,
    pub version_minor: u8,
    pub mode: u8,
    /// Logical vector length in lanes. `0` selects the runtime default.
    pub vector_length: u8,
    pub max_cycles: u64,
    /// ABI version for syscall table and pointer-ABI schema.
    pub abi_version: u8,
}

/// Result of parsing metadata and locating the code segment inside a program artifact.
#[derive(Clone, Debug)]
pub struct ParsedProgramMetadata {
    pub metadata: ProgramMetadata,
    /// Number of bytes occupied by the metadata header.
    pub header_len: usize,
    /// Absolute offset within the artifact where executable code begins.
    pub code_offset: usize,
    /// Decoded embedded contract interface for self-describing 1.1 contract artifacts.
    pub contract_interface: Option<EmbeddedContractInterfaceV1>,
    /// Optional compiler debug metadata for self-describing 1.1 contract artifacts.
    pub contract_debug: Option<EmbeddedContractDebugInfoV1>,
    /// Validated location metadata for the optional indexed literal table.
    pub literal_section: Option<ParsedLiteralSection>,
}

/// Structurally validated byte ranges for an `LTLB` indexed literal section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedLiteralSection {
    /// Absolute artifact offset of the `LTLB` marker.
    pub start: usize,
    /// Absolute artifact offset of the first `u64` literal-table descriptor.
    pub entries_start: usize,
    /// Absolute artifact offset of the first typed literal payload.
    pub data_start: usize,
    /// Exclusive absolute artifact offset of literal data, before alignment padding.
    pub data_end: usize,
    /// Number of indexed literal entries.
    pub count: usize,
    /// Absolute artifact offset where executable code begins.
    pub code_offset: usize,
}

impl ParsedProgramMetadata {
    /// Length of the ordered prefix sections placed between the header and executable code.
    pub fn prefix_len(&self) -> usize {
        self.code_offset.saturating_sub(self.header_len)
    }
}

impl ProgramMetadata {
    pub fn parse(bytes: &[u8]) -> Result<ParsedProgramMetadata, VMError> {
        if bytes.len() < HEADER_SIZE {
            return Err(VMError::InvalidMetadata);
        }
        let magic = &bytes[0..4];
        let version_major = bytes[4];
        if magic != MAGIC {
            return Err(VMError::InvalidMetadata);
        }
        let abi_version = bytes[16];
        let abi_hash: [u8; 32] = bytes[17..49]
            .try_into()
            .map_err(|_| VMError::InvalidMetadata)?;
        let header_len = HEADER_SIZE;
        let version_minor = bytes[5];
        let mode = bytes[6];
        let vector_length = bytes[7];
        let max_cycles_bytes: [u8; 8] = bytes[8..16]
            .try_into()
            .map_err(|_| VMError::InvalidMetadata)?;
        let max_cycles = u64::from_le_bytes(max_cycles_bytes);

        // Validate header fields according to the current implementation policy.
        // - Accept generic version 1.0 and 1.1 headers.
        // - Self-describing contract artifacts remain a 1.1-only concept and are
        //   validated by higher-level artifact verification.
        // - Mode must not contain unknown bits (only ZK, VECTOR, HTM).
        // - `vector_length` is either 0 (use runtime default) or 1..=64.
        // - ABI version is carried as-is; admission enforces allowed values.
        const KNOWN_MODE_BITS: u8 = mode::ZK | mode::VECTOR | mode::HTM;
        if version_major != 1 {
            return Err(VMError::InvalidMetadata);
        }
        if !matches!(version_minor, 0 | 1) {
            return Err(VMError::InvalidMetadata);
        }
        if mode & !KNOWN_MODE_BITS != 0 {
            return Err(VMError::InvalidMetadata);
        }
        if vector_length > VECTOR_LENGTH_MAX {
            return Err(VMError::InvalidMetadata);
        }
        if abi_version == 1 {
            let expected = crate::syscalls::compute_abi_hash(crate::SyscallPolicy::AbiV1);
            if abi_hash != expected {
                return Err(VMError::ArtifactAbiHashMismatch {
                    expected,
                    actual: abi_hash,
                });
            }
        }
        // Note: vector_length may be non-zero even if VECTOR flag is off; the
        // host/runtime may ignore it depending on policy.
        let mut code_offset = header_len;
        let mut contract_interface = None;
        let mut contract_debug = None;
        let mut literal_section = None;

        if bytes.len() >= code_offset + 4
            && bytes[code_offset..code_offset + 4] == CONTRACT_INTERFACE_SECTION_MAGIC
        {
            let (decoded_interface, next_offset) =
                parse_contract_interface_section(bytes, header_len)?;
            contract_interface = Some(decoded_interface);
            code_offset = next_offset;
        }

        if bytes.len() >= code_offset + 4
            && bytes[code_offset..code_offset + 4] == CONTRACT_DEBUG_SECTION_MAGIC
        {
            let (decoded_debug, next_offset) = parse_contract_debug_section(bytes, code_offset)?;
            contract_debug = Some(decoded_debug);
            code_offset = next_offset;
        }

        // Optional literal section begins immediately after the header for
        // generic 1.1 artifacts, or after the ordered `CNTR`/`DBG1` sections
        // present in self-describing contract artifacts.
        if bytes.len() >= code_offset + 4
            && bytes[code_offset..code_offset + 4] == LITERAL_SECTION_MAGIC
        {
            let parsed = parse_literal_section(bytes, code_offset, header_len)?;
            code_offset = parsed.code_offset;
            literal_section = Some(parsed);
        } else if bytes.len() >= header_len + 4 {
            // Reject prefixed layouts that insert zero padding before the literal table marker.
            let max_scan = header_len + 32;
            let limit = bytes.len().saturating_sub(4);
            let end = max_scan.min(limit);
            let mut idx = header_len;
            while idx <= end {
                if bytes[idx..idx + 4] == LITERAL_SECTION_MAGIC {
                    let pad = &bytes[header_len..idx];
                    if pad.iter().all(|b| *b == 0) {
                        return Err(VMError::InvalidMetadata);
                    }
                    break;
                } else if bytes[idx] != 0 {
                    break;
                }
                idx += 1;
            }
        }
        Ok(ParsedProgramMetadata {
            metadata: Self {
                version_major,
                version_minor,
                mode,
                vector_length,
                max_cycles,
                abi_version,
            },
            header_len,
            code_offset,
            contract_interface,
            contract_debug,
            literal_section,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(MAGIC);
        v.push(self.version_major);
        v.push(self.version_minor);
        v.push(self.mode);
        v.push(self.vector_length);
        v.extend_from_slice(&self.max_cycles.to_le_bytes());
        v.push(self.abi_version);
        let abi_hash = match self.abi_version {
            1 => crate::syscalls::compute_abi_hash(crate::SyscallPolicy::AbiV1),
            _ => [0; 32],
        };
        v.extend_from_slice(&abi_hash);
        v
    }

    /// Construct a default header for a specific `version_major.version_minor`
    /// and `abi_version`. Other fields are set to zero.
    pub fn default_for(version_major: u8, version_minor: u8, abi_version: u8) -> Self {
        Self {
            version_major,
            version_minor,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version,
        }
    }
}

impl Default for ProgramMetadata {
    fn default() -> Self {
        Self {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        }
    }
}

fn parse_contract_interface_section(
    bytes: &[u8],
    start: usize,
) -> Result<(EmbeddedContractInterfaceV1, usize), VMError> {
    if bytes.len() < start + CONTRACT_INTERFACE_SECTION_HEADER_SIZE {
        return Err(VMError::InvalidMetadata);
    }
    if bytes[start..start + 4] != CONTRACT_INTERFACE_SECTION_MAGIC {
        return Err(VMError::InvalidMetadata);
    }
    let len_bytes: [u8; 4] = bytes[start + 4..start + 8]
        .try_into()
        .map_err(|_| VMError::InvalidMetadata)?;
    let payload_len = u32::from_le_bytes(len_bytes) as usize;
    let payload_start = start + CONTRACT_INTERFACE_SECTION_HEADER_SIZE;
    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or(VMError::InvalidMetadata)?;
    if payload_end > bytes.len() {
        return Err(VMError::InvalidMetadata);
    }
    let decoded = norito::decode_from_bytes::<EmbeddedContractInterfaceV1>(
        &bytes[payload_start..payload_end],
    )
    .map_err(|_| VMError::InvalidMetadata)?;
    Ok((decoded, payload_end))
}

fn parse_contract_debug_section(
    bytes: &[u8],
    start: usize,
) -> Result<(EmbeddedContractDebugInfoV1, usize), VMError> {
    if bytes.len() < start + CONTRACT_DEBUG_SECTION_HEADER_SIZE {
        return Err(VMError::InvalidMetadata);
    }
    if bytes[start..start + 4] != CONTRACT_DEBUG_SECTION_MAGIC {
        return Err(VMError::InvalidMetadata);
    }
    let len_bytes: [u8; 4] = bytes[start + 4..start + 8]
        .try_into()
        .map_err(|_| VMError::InvalidMetadata)?;
    let payload_len = u32::from_le_bytes(len_bytes) as usize;
    let payload_start = start + CONTRACT_DEBUG_SECTION_HEADER_SIZE;
    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or(VMError::InvalidMetadata)?;
    if payload_end > bytes.len() {
        return Err(VMError::InvalidMetadata);
    }
    let decoded = norito::decode_from_bytes::<EmbeddedContractDebugInfoV1>(
        &bytes[payload_start..payload_end],
    )
    .map_err(|_| VMError::InvalidMetadata)?;
    Ok((decoded, payload_end))
}

fn parse_literal_section(
    bytes: &[u8],
    start: usize,
    header_len: usize,
) -> Result<ParsedLiteralSection, VMError> {
    if bytes.len() < start + 16 {
        return Err(VMError::InvalidMetadata);
    }
    let count_bytes: [u8; 4] = bytes[start + 4..start + 8]
        .try_into()
        .map_err(|_| VMError::InvalidMetadata)?;
    let post_bytes: [u8; 4] = bytes[start + 8..start + 12]
        .try_into()
        .map_err(|_| VMError::InvalidMetadata)?;
    let data_bytes: [u8; 4] = bytes[start + 12..start + 16]
        .try_into()
        .map_err(|_| VMError::InvalidMetadata)?;
    let lit_count = u32::from_le_bytes(count_bytes) as usize;
    let post_pad = u32::from_le_bytes(post_bytes) as usize;
    let data_len = u32::from_le_bytes(data_bytes) as usize;
    if lit_count > usize::from(u16::MAX) + 1 || post_pad > 3 {
        return Err(VMError::InvalidMetadata);
    }
    let entries_len = lit_count.checked_mul(8).ok_or(VMError::InvalidMetadata)?;
    let data_start = start
        .checked_add(16)
        .and_then(|offset| offset.checked_add(entries_len))
        .ok_or(VMError::InvalidMetadata)?;
    let data_end = data_start
        .checked_add(data_len)
        .ok_or(VMError::InvalidMetadata)?;
    let code_offset = data_end
        .checked_add(post_pad)
        .ok_or(VMError::InvalidMetadata)?;
    if code_offset > bytes.len() || start < header_len {
        return Err(VMError::InvalidMetadata);
    }
    let unpadded_len_from_header = start
        .checked_sub(header_len)
        .and_then(|prefix_len| prefix_len.checked_add(16))
        .and_then(|len| len.checked_add(entries_len))
        .and_then(|len| len.checked_add(data_len))
        .ok_or(VMError::InvalidMetadata)?;
    let expected_pad = (4 - (unpadded_len_from_header % 4)) % 4;
    if post_pad != expected_pad {
        return Err(VMError::InvalidMetadata);
    }
    if bytes[data_end..code_offset].iter().any(|byte| *byte != 0) {
        return Err(VMError::InvalidMetadata);
    }
    Ok(ParsedLiteralSection {
        start,
        entries_start: start + 16,
        data_start,
        data_end,
        count: lit_count,
        code_offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_descriptors_roundtrip_kind_and_full_offset_domain() {
        for kind in [LiteralKindV1::PointerTlv, LiteralKindV1::I64] {
            for offset in [0, 1, 0x00ff_ffff, LITERAL_OFFSET_MASK] {
                let raw = encode_literal_descriptor(kind, offset).expect("encodable offset");
                assert_eq!(decode_literal_descriptor(raw), Ok((kind, offset)));
            }
        }
        assert!(encode_literal_descriptor(LiteralKindV1::I64, LITERAL_OFFSET_MASK + 1).is_none());
        assert!(decode_literal_descriptor(0xff00_0000_0000_0000).is_err());
    }

    #[test]
    fn contract_code_hash_binds_header_and_body() {
        let mut artifact = vec![0_u8; HEADER_SIZE + 4];
        let original = contract_code_hash(&artifact);

        artifact[7] ^= 1;
        assert_ne!(contract_code_hash(&artifact), original);
        artifact[7] ^= 1;

        artifact[HEADER_SIZE] ^= 1;
        assert_ne!(contract_code_hash(&artifact), original);
        artifact[HEADER_SIZE] ^= 1;

        assert_eq!(contract_code_hash(&artifact), original);
        assert_ne!(original, iroha_crypto::Hash::new(&artifact));
    }

    #[test]
    fn literal_section_parse_reports_validated_ranges() {
        let mut artifact = ProgramMetadata::default().encode();
        let start = artifact.len();
        artifact.extend_from_slice(&LITERAL_SECTION_MAGIC);
        artifact.extend_from_slice(&1u32.to_le_bytes());
        artifact.extend_from_slice(&1u32.to_le_bytes());
        artifact.extend_from_slice(&3u32.to_le_bytes());
        artifact.extend_from_slice(&24u64.to_le_bytes());
        artifact.extend_from_slice(&[1, 2, 3]);
        artifact.push(0);
        artifact.extend_from_slice(&0x4900_0000u32.to_le_bytes());

        let parsed = ProgramMetadata::parse(&artifact).expect("literal section parses");
        let section = parsed.literal_section.expect("literal section metadata");
        assert_eq!(section.start, start);
        assert_eq!(section.entries_start, start + 16);
        assert_eq!(section.data_start, start + 24);
        assert_eq!(section.data_end, start + 27);
        assert_eq!(section.code_offset, start + 28);
        assert_eq!(section.count, 1);
        assert_eq!(parsed.code_offset, section.code_offset);
        assert_eq!(parsed.prefix_len(), section.code_offset - HEADER_SIZE);
    }

    fn nested_state_type() -> EmbeddedStateType {
        EmbeddedStateType::Struct {
            name: "WalletState".to_owned(),
            fields: vec![
                EmbeddedStateFieldDescriptor {
                    name: "balances".to_owned(),
                    ty: EmbeddedStateType::StateMap {
                        key: Box::new(EmbeddedStateType::AccountId),
                        value: Box::new(EmbeddedStateType::Tuple(vec![
                            EmbeddedStateType::AssetDefinitionId,
                            EmbeddedStateType::Quantity,
                        ])),
                    },
                },
                EmbeddedStateFieldDescriptor {
                    name: "metadata".to_owned(),
                    ty: EmbeddedStateType::Option(Box::new(EmbeddedStateType::Struct {
                        name: "Metadata".to_owned(),
                        fields: vec![EmbeddedStateFieldDescriptor {
                            name: "active".to_owned(),
                            ty: EmbeddedStateType::Result {
                                ok: Box::new(EmbeddedStateType::Bool),
                                err: Box::new(EmbeddedStateType::String),
                            },
                        }],
                    })),
                },
                EmbeddedStateFieldDescriptor {
                    name: "recent_amounts".to_owned(),
                    ty: EmbeddedStateType::List {
                        element: Box::new(EmbeddedStateType::Option(Box::new(
                            EmbeddedStateType::Quantity,
                        ))),
                        capacity: 16,
                    },
                },
            ],
        }
    }

    #[test]
    fn embedded_state_field_descriptor_roundtrips() {
        let value = EmbeddedStateFieldDescriptor {
            name: "root".to_owned(),
            ty: nested_state_type(),
        };

        let bytes = norito::to_bytes(&value).expect("encode embedded state field");
        let decoded: EmbeddedStateFieldDescriptor =
            norito::decode_from_bytes(&bytes).expect("decode embedded state field");

        assert_eq!(decoded, value);
    }

    #[test]
    fn embedded_state_type_roundtrips() {
        let value = nested_state_type();

        let bytes = norito::to_bytes(&value).expect("encode embedded state type");
        let decoded: EmbeddedStateType =
            norito::decode_from_bytes(&bytes).expect("decode embedded state type");

        assert_eq!(decoded, value);
    }

    #[test]
    fn embedded_state_type_tags_and_nominal_schema_names_are_stable() {
        let variants = vec![
            EmbeddedStateType::Int,
            EmbeddedStateType::Decimal,
            EmbeddedStateType::Quantity,
            EmbeddedStateType::Bool,
            EmbeddedStateType::String,
            EmbeddedStateType::Bytes,
            EmbeddedStateType::DataSpaceId,
            EmbeddedStateType::AccountId,
            EmbeddedStateType::AssetDefinitionId,
            EmbeddedStateType::AssetId,
            EmbeddedStateType::NftId,
            EmbeddedStateType::DomainId,
            EmbeddedStateType::Name,
            EmbeddedStateType::Json,
            EmbeddedStateType::Tuple(vec![EmbeddedStateType::Int, EmbeddedStateType::Decimal]),
            EmbeddedStateType::Struct {
                name: "Stable".to_owned(),
                fields: vec![EmbeddedStateFieldDescriptor {
                    name: "value".to_owned(),
                    ty: EmbeddedStateType::Quantity,
                }],
            },
            EmbeddedStateType::StateMap {
                key: Box::new(EmbeddedStateType::AccountId),
                value: Box::new(EmbeddedStateType::Quantity),
            },
            EmbeddedStateType::Option(Box::new(EmbeddedStateType::Int)),
            EmbeddedStateType::Result {
                ok: Box::new(EmbeddedStateType::Int),
                err: Box::new(EmbeddedStateType::String),
            },
            EmbeddedStateType::List {
                element: Box::new(EmbeddedStateType::Quantity),
                capacity: 64,
            },
        ];

        for (expected_tag, value) in (0_u8..).zip(variants) {
            assert_eq!(value.wire_tag(), expected_tag);
            let payload =
                encode_embedded_state_type_payload(&value).expect("encode stable CNTR type");
            assert_eq!(payload.first(), Some(&expected_tag));
        }

        assert_eq!(
            <EmbeddedStateType as NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(EMBEDDED_STATE_TYPE_SCHEMA_NAME_V1)
        );
        assert_eq!(
            <EmbeddedContractInterfaceV1 as NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(CONTRACT_INTERFACE_SCHEMA_NAME_V1)
        );
    }

    #[test]
    fn embedded_state_type_nesting_is_bounded_before_recursive_work() {
        std::thread::Builder::new()
            .name("embedded-state-depth-boundary".to_owned())
            .stack_size(128 * 1024)
            .spawn(|| {
                fn option_chain(wrappers: usize) -> EmbeddedStateType {
                    (0..wrappers).fold(EmbeddedStateType::Bool, |inner, _| {
                        EmbeddedStateType::Option(Box::new(inner))
                    })
                }

                fn raw_option_payload(wrappers: usize) -> Vec<u8> {
                    let mut payload = vec![EMBEDDED_STATE_TYPE_TAG_BOOL];
                    for _ in 0..wrappers {
                        let mut outer = vec![EMBEDDED_STATE_TYPE_TAG_OPTION];
                        encode_embedded_state_owned_child(&payload, &mut outer)
                            .expect("serialize adversarial nested payload");
                        payload = outer;
                    }
                    payload
                }

                let accepted_wrappers = MAX_EMBEDDED_STATE_TYPE_DEPTH_V1 - 1;
                encode_embedded_state_type_payload(&option_chain(accepted_wrappers))
                    .expect("the exact nesting budget remains valid");
                decode_embedded_state_type_payload(&raw_option_payload(accepted_wrappers))
                    .expect("the exact decoding budget remains valid");

                let error = encode_embedded_state_type_payload(&option_chain(
                    MAX_EMBEDDED_STATE_TYPE_DEPTH_V1,
                ))
                .expect_err("encoding above the state-type nesting budget must fail");
                assert!(error.to_string().contains("nesting exceeds 256 levels"));

                let error = decode_embedded_state_type_payload(&raw_option_payload(
                    MAX_EMBEDDED_STATE_TYPE_DEPTH_V1,
                ))
                .expect_err(
                    "decoding above the state-type nesting budget must fail before admission",
                );
                assert!(error.to_string().contains("nesting exceeds 256 levels"));
                decode_embedded_state_type_payload(&[EMBEDDED_STATE_TYPE_TAG_BOOL])
                    .expect("a rejected payload must not poison the next decode");
            })
            .expect("spawn constrained-stack state-schema test")
            .join()
            .expect("state-schema depth checks must not overflow the native stack");
    }

    #[test]
    fn iterative_state_decoder_rejects_malformed_or_forbidden_nested_children() {
        fn option_payload(child: Vec<u8>) -> Vec<u8> {
            let mut payload = vec![EMBEDDED_STATE_TYPE_TAG_OPTION];
            encode_embedded_state_owned_child(&child, &mut payload)
                .expect("serialize adversarial option child");
            payload
        }

        for child in [
            Vec::new(),
            vec![EMBEDDED_STATE_TYPE_TAG_BOOL, 0],
            vec![u8::MAX],
        ] {
            decode_embedded_state_type_payload(&option_payload(child))
                .expect_err("malformed nested type payload must reject");
        }

        let state_map = EmbeddedStateType::StateMap {
            key: Box::new(EmbeddedStateType::AccountId),
            value: Box::new(EmbeddedStateType::Quantity),
        };
        let encoded_map =
            encode_embedded_state_type_payload(&state_map).expect("encode root StateMap");
        let mut forbidden_list = vec![EMBEDDED_STATE_TYPE_TAG_LIST];
        encode_embedded_state_owned_child(&encoded_map, &mut forbidden_list)
            .expect("embed StateMap payload without invoking List validation");
        1_u8.serialize(&mut forbidden_list)
            .expect("serialize List capacity");
        let error = decode_embedded_state_type_payload(&forbidden_list)
            .expect_err("decoded List element cannot hide a StateMap resource handle");
        assert!(error.to_string().contains("resource handles"));

        decode_embedded_state_type_payload(&[EMBEDDED_STATE_TYPE_TAG_QUANTITY])
            .expect("rejections must not poison a later valid decode");
    }

    #[test]
    fn embedded_list_uses_stable_tag_and_validates_capacity() {
        let value = EmbeddedStateType::List {
            element: Box::new(EmbeddedStateType::Quantity),
            capacity: 64,
        };
        let mut payload = encode_embedded_state_type_payload(&value).expect("encode List schema");
        assert_eq!(payload[0], EMBEDDED_STATE_TYPE_TAG_LIST);

        *payload.last_mut().expect("capacity byte") = 0;
        let err = decode_embedded_state_type_payload(&payload).expect_err("reject zero capacity");
        assert!(err.to_string().contains("capacity must be in 1..=64"));

        let invalid = EmbeddedStateType::List {
            element: Box::new(EmbeddedStateType::Bool),
            capacity: 65,
        };
        let err = encode_embedded_state_type_payload(&invalid).expect_err("reject capacity 65");
        assert!(err.to_string().contains("capacity must be in 1..=64"));
    }

    #[test]
    fn embedded_list_rejects_nested_resource_handles() {
        let value = EmbeddedStateType::List {
            element: Box::new(EmbeddedStateType::Option(Box::new(
                EmbeddedStateType::StateMap {
                    key: Box::new(EmbeddedStateType::AccountId),
                    value: Box::new(EmbeddedStateType::Quantity),
                },
            ))),
            capacity: 4,
        };

        let err = encode_embedded_state_type_payload(&value)
            .expect_err("nested StateMap cannot cross a List boundary");
        assert!(err.to_string().contains("resource handles"));
    }

    #[test]
    fn contract_interface_section_roundtrips_nested_states() {
        let interface = EmbeddedContractInterfaceV1 {
            seiyaku_name: "TestContract".to_owned(),
            compiler_fingerprint: "metadata-tests".to_owned(),
            abi_hash: crate::syscalls::compute_abi_hash(crate::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![EmbeddedEntrypointDescriptor {
                name: "main".to_owned(),
                kind: EntryPointKind::View,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            error_codes: vec![ContractErrorCodeDescriptor {
                namespace: "PaymentError".to_owned(),
                name: "Unauthorized".to_owned(),
                code: 1001,
            }],
            states: vec![EmbeddedStateDescriptor {
                name: "wallet".to_owned(),
                ty: nested_state_type(),
            }],
        };

        let section = interface.encode_section();
        let (decoded, next_offset) =
            parse_contract_interface_section(&section, 0).expect("parse CNTR section");

        assert_eq!(next_offset, section.len());
        assert_eq!(decoded, interface);

        let mut artifact = ProgramMetadata::default().encode();
        artifact.extend_from_slice(&section);
        artifact.extend_from_slice(&0x4900_0000_u32.to_le_bytes());
        let parsed = ProgramMetadata::parse(&artifact).expect("parse artifact with CNTR prefix");
        assert_eq!(parsed.header_len, HEADER_SIZE);
        assert_eq!(parsed.prefix_len(), section.len());
        assert_eq!(parsed.code_offset, HEADER_SIZE + section.len());
    }
}
