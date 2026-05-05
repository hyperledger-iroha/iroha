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
    AccessSetHints, EntryPointKind, EntrypointDescriptor, KotobaTranslationEntry, TriggerDescriptor,
};
use norito::{
    Decode, Encode,
    core::{Archived, DecodeFromSlice, Error as NoritoError, NoritoDeserialize, NoritoSerialize},
};

/// Maximum accepted logical vector length for admission.
pub const VECTOR_LENGTH_MAX: u8 = 64;

/// Magic prefix identifying IVM bytecode.
pub const MAGIC: &[u8; 4] = b"IVM\0";
pub const HEADER_SIZE: usize = 17;

/// Literal table section marker placed immediately after the metadata header
/// when compiled bytecode includes literal fixups.
pub const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";
/// Embedded contract interface section marker used by self-describing contract artifacts.
pub const CONTRACT_INTERFACE_SECTION_MAGIC: [u8; 4] = *b"CNTR";
/// Embedded contract debug section marker used by self-describing contract artifacts.
pub const CONTRACT_DEBUG_SECTION_MAGIC: [u8; 4] = *b"DBG1";
/// Embedded contract feature bit: zero-knowledge mode.
pub const CONTRACT_FEATURE_BIT_ZK: u64 = 1 << 0;
/// Embedded contract feature bit: vector mode.
pub const CONTRACT_FEATURE_BIT_VECTOR: u64 = 1 << 1;
/// Bitmask of all currently supported embedded contract feature bits.
pub const CONTRACT_FEATURE_KNOWN_BITS: u64 = CONTRACT_FEATURE_BIT_ZK | CONTRACT_FEATURE_BIT_VECTOR;

const CONTRACT_INTERFACE_SECTION_HEADER_SIZE: usize = 8;
const CONTRACT_DEBUG_SECTION_HEADER_SIZE: usize = 8;

/// Artifact-local entrypoint metadata carried inside the required `CNTR` section.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedEntrypointDescriptor {
    pub name: String,
    pub kind: EntryPointKind,
    pub params: Vec<iroha_data_model::smart_contract::manifest::EntrypointParamDescriptor>,
    pub return_type: Option<String>,
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
            return_type: self.return_type.clone(),
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
    FixedU128,
    Amount,
    Balance,
    Bool,
    String,
    Blob,
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
    Map {
        key: Box<EmbeddedStateType>,
        value: Box<EmbeddedStateType>,
    },
}

const EMBEDDED_STATE_TYPE_TAG_INT: u8 = 0;
const EMBEDDED_STATE_TYPE_TAG_FIXED_U128: u8 = 1;
const EMBEDDED_STATE_TYPE_TAG_AMOUNT: u8 = 2;
const EMBEDDED_STATE_TYPE_TAG_BALANCE: u8 = 3;
const EMBEDDED_STATE_TYPE_TAG_BOOL: u8 = 4;
const EMBEDDED_STATE_TYPE_TAG_STRING: u8 = 5;
const EMBEDDED_STATE_TYPE_TAG_BLOB: u8 = 6;
const EMBEDDED_STATE_TYPE_TAG_BYTES: u8 = 7;
const EMBEDDED_STATE_TYPE_TAG_DATASPACE_ID: u8 = 8;
const EMBEDDED_STATE_TYPE_TAG_ACCOUNT_ID: u8 = 9;
const EMBEDDED_STATE_TYPE_TAG_ASSET_DEFINITION_ID: u8 = 10;
const EMBEDDED_STATE_TYPE_TAG_ASSET_ID: u8 = 11;
const EMBEDDED_STATE_TYPE_TAG_NFT_ID: u8 = 12;
const EMBEDDED_STATE_TYPE_TAG_DOMAIN_ID: u8 = 13;
const EMBEDDED_STATE_TYPE_TAG_NAME: u8 = 14;
const EMBEDDED_STATE_TYPE_TAG_JSON: u8 = 15;
const EMBEDDED_STATE_TYPE_TAG_TUPLE: u8 = 16;
const EMBEDDED_STATE_TYPE_TAG_STRUCT: u8 = 17;
const EMBEDDED_STATE_TYPE_TAG_MAP: u8 = 18;

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
    let mut payload = Vec::new();
    match value {
        EmbeddedStateType::Int => EMBEDDED_STATE_TYPE_TAG_INT.serialize(&mut payload)?,
        EmbeddedStateType::FixedU128 => {
            EMBEDDED_STATE_TYPE_TAG_FIXED_U128.serialize(&mut payload)?
        }
        EmbeddedStateType::Amount => EMBEDDED_STATE_TYPE_TAG_AMOUNT.serialize(&mut payload)?,
        EmbeddedStateType::Balance => EMBEDDED_STATE_TYPE_TAG_BALANCE.serialize(&mut payload)?,
        EmbeddedStateType::Bool => EMBEDDED_STATE_TYPE_TAG_BOOL.serialize(&mut payload)?,
        EmbeddedStateType::String => EMBEDDED_STATE_TYPE_TAG_STRING.serialize(&mut payload)?,
        EmbeddedStateType::Blob => EMBEDDED_STATE_TYPE_TAG_BLOB.serialize(&mut payload)?,
        EmbeddedStateType::Bytes => EMBEDDED_STATE_TYPE_TAG_BYTES.serialize(&mut payload)?,
        EmbeddedStateType::DataSpaceId => {
            EMBEDDED_STATE_TYPE_TAG_DATASPACE_ID.serialize(&mut payload)?
        }
        EmbeddedStateType::AccountId => {
            EMBEDDED_STATE_TYPE_TAG_ACCOUNT_ID.serialize(&mut payload)?
        }
        EmbeddedStateType::AssetDefinitionId => {
            EMBEDDED_STATE_TYPE_TAG_ASSET_DEFINITION_ID.serialize(&mut payload)?
        }
        EmbeddedStateType::AssetId => EMBEDDED_STATE_TYPE_TAG_ASSET_ID.serialize(&mut payload)?,
        EmbeddedStateType::NftId => EMBEDDED_STATE_TYPE_TAG_NFT_ID.serialize(&mut payload)?,
        EmbeddedStateType::DomainId => EMBEDDED_STATE_TYPE_TAG_DOMAIN_ID.serialize(&mut payload)?,
        EmbeddedStateType::Name => EMBEDDED_STATE_TYPE_TAG_NAME.serialize(&mut payload)?,
        EmbeddedStateType::Json => EMBEDDED_STATE_TYPE_TAG_JSON.serialize(&mut payload)?,
        EmbeddedStateType::Tuple(values) => {
            EMBEDDED_STATE_TYPE_TAG_TUPLE.serialize(&mut payload)?;
            values.serialize(&mut payload)?;
        }
        EmbeddedStateType::Struct { name, fields } => {
            EMBEDDED_STATE_TYPE_TAG_STRUCT.serialize(&mut payload)?;
            name.serialize(&mut payload)?;
            fields.serialize(&mut payload)?;
        }
        EmbeddedStateType::Map { key, value } => {
            EMBEDDED_STATE_TYPE_TAG_MAP.serialize(&mut payload)?;
            key.serialize(&mut payload)?;
            value.serialize(&mut payload)?;
        }
    }
    Ok(payload)
}

fn decode_embedded_state_type_payload(encoded: &[u8]) -> Result<EmbeddedStateType, NoritoError> {
    let (tag, tag_used) = <u8 as DecodeFromSlice>::decode_from_slice(encoded)?;
    let payload = &encoded[tag_used..];
    let (value, consumed) = match tag {
        EMBEDDED_STATE_TYPE_TAG_INT => (EmbeddedStateType::Int, 0),
        EMBEDDED_STATE_TYPE_TAG_FIXED_U128 => (EmbeddedStateType::FixedU128, 0),
        EMBEDDED_STATE_TYPE_TAG_AMOUNT => (EmbeddedStateType::Amount, 0),
        EMBEDDED_STATE_TYPE_TAG_BALANCE => (EmbeddedStateType::Balance, 0),
        EMBEDDED_STATE_TYPE_TAG_BOOL => (EmbeddedStateType::Bool, 0),
        EMBEDDED_STATE_TYPE_TAG_STRING => (EmbeddedStateType::String, 0),
        EMBEDDED_STATE_TYPE_TAG_BLOB => (EmbeddedStateType::Blob, 0),
        EMBEDDED_STATE_TYPE_TAG_BYTES => (EmbeddedStateType::Bytes, 0),
        EMBEDDED_STATE_TYPE_TAG_DATASPACE_ID => (EmbeddedStateType::DataSpaceId, 0),
        EMBEDDED_STATE_TYPE_TAG_ACCOUNT_ID => (EmbeddedStateType::AccountId, 0),
        EMBEDDED_STATE_TYPE_TAG_ASSET_DEFINITION_ID => (EmbeddedStateType::AssetDefinitionId, 0),
        EMBEDDED_STATE_TYPE_TAG_ASSET_ID => (EmbeddedStateType::AssetId, 0),
        EMBEDDED_STATE_TYPE_TAG_NFT_ID => (EmbeddedStateType::NftId, 0),
        EMBEDDED_STATE_TYPE_TAG_DOMAIN_ID => (EmbeddedStateType::DomainId, 0),
        EMBEDDED_STATE_TYPE_TAG_NAME => (EmbeddedStateType::Name, 0),
        EMBEDDED_STATE_TYPE_TAG_JSON => (EmbeddedStateType::Json, 0),
        EMBEDDED_STATE_TYPE_TAG_TUPLE => {
            let (values, values_used) =
                <Vec<EmbeddedStateType> as DecodeFromSlice>::decode_from_slice(payload)?;
            (EmbeddedStateType::Tuple(values), values_used)
        }
        EMBEDDED_STATE_TYPE_TAG_STRUCT => {
            let (name, name_used) = <String as DecodeFromSlice>::decode_from_slice(payload)?;
            let (fields, fields_used) =
                <Vec<EmbeddedStateFieldDescriptor> as DecodeFromSlice>::decode_from_slice(
                    &payload[name_used..],
                )?;
            (
                EmbeddedStateType::Struct { name, fields },
                name_used + fields_used,
            )
        }
        EMBEDDED_STATE_TYPE_TAG_MAP => {
            let (key, key_used) =
                <Box<EmbeddedStateType> as DecodeFromSlice>::decode_from_slice(payload)?;
            let (value, value_used) =
                <Box<EmbeddedStateType> as DecodeFromSlice>::decode_from_slice(
                    &payload[key_used..],
                )?;
            (EmbeddedStateType::Map { key, value }, key_used + value_used)
        }
        other => {
            return Err(NoritoError::invalid_tag(
                "EmbeddedStateType::try_deserialize",
                other,
            ));
        }
    };
    expect_payload_consumed(consumed, payload.len(), "EmbeddedStateType")?;
    Ok(value)
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
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), NoritoError> {
        let encoded = encode_embedded_state_type_payload(self)?;
        encoded.serialize(&mut writer)
    }
}

impl<'a> NoritoDeserialize<'a> for EmbeddedStateType {
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

/// Contract-level durable state declaration descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedStateDescriptor {
    pub name: String,
    pub ty: EmbeddedStateType,
}

/// Decoded payload of the required `CNTR` section carried by contract artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedContractInterfaceV1 {
    pub compiler_fingerprint: String,
    pub features_bitmap: u64,
    pub access_set_hints: Option<AccessSetHints>,
    pub kotoba: Vec<KotobaTranslationEntry>,
    pub entrypoints: Vec<EmbeddedEntrypointDescriptor>,
    pub states: Vec<EmbeddedStateDescriptor>,
}

/// Source location emitted for function-level compiler debug metadata.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EmbeddedSourceLocation {
    #[norito(default)]
    pub source_path: Option<String>,
    pub line: u32,
    pub column: u32,
}

/// Function-level source mapping emitted inside the optional `DBG1` section.
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
    /// Logical vector length in lanes. `0` selects the maximum supported value.
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
}

impl ParsedProgramMetadata {
    /// Length of the ordered prefix sections placed between the header and executable code.
    pub fn prefix_len(&self) -> usize {
        self.code_offset.saturating_sub(self.header_len)
    }

    /// Backward-compatible alias for callers that still refer to the old literal-only prefix.
    pub fn literal_prefix_len(&self) -> usize {
        self.prefix_len()
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
        // - `vector_length` is advisory and may be set regardless of the VECTOR bit.
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
        // Note: vector_length may be non-zero even if VECTOR flag is off; the
        // host/runtime may clamp or ignore it depending on policy.
        let mut code_offset = header_len;
        let mut contract_interface = None;
        let mut contract_debug = None;

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
        // generic 1.1 artifacts, or immediately after the `CNTR` section when
        // present in self-describing contract artifacts.
        if bytes.len() >= code_offset + 4
            && bytes[code_offset..code_offset + 4] == LITERAL_SECTION_MAGIC
        {
            code_offset = parse_literal_section_end(bytes, code_offset)?;
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

fn parse_literal_section_end(bytes: &[u8], start: usize) -> Result<usize, VMError> {
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
    let lit_len = lit_count
        .checked_mul(8)
        .and_then(|n| n.checked_add(16))
        .and_then(|n| n.checked_add(post_pad))
        .and_then(|n| n.checked_add(data_len))
        .ok_or(VMError::InvalidMetadata)?;
    let code_offset = start.checked_add(lit_len).ok_or(VMError::InvalidMetadata)?;
    if code_offset > bytes.len() {
        return Err(VMError::InvalidMetadata);
    }
    Ok(code_offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_state_type() -> EmbeddedStateType {
        EmbeddedStateType::Struct {
            name: "WalletState".to_owned(),
            fields: vec![
                EmbeddedStateFieldDescriptor {
                    name: "balances".to_owned(),
                    ty: EmbeddedStateType::Map {
                        key: Box::new(EmbeddedStateType::AccountId),
                        value: Box::new(EmbeddedStateType::Tuple(vec![
                            EmbeddedStateType::AssetDefinitionId,
                            EmbeddedStateType::Balance,
                        ])),
                    },
                },
                EmbeddedStateFieldDescriptor {
                    name: "metadata".to_owned(),
                    ty: EmbeddedStateType::Struct {
                        name: "Metadata".to_owned(),
                        fields: vec![EmbeddedStateFieldDescriptor {
                            name: "active".to_owned(),
                            ty: EmbeddedStateType::Bool,
                        }],
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
    fn contract_interface_section_roundtrips_nested_states() {
        let interface = EmbeddedContractInterfaceV1 {
            compiler_fingerprint: "metadata-tests".to_owned(),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![EmbeddedEntrypointDescriptor {
                name: "main".to_owned(),
                kind: EntryPointKind::Public,
                params: Vec::new(),
                return_type: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
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
    }
}
