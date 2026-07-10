//! One-shot decoding for compiler-generated public entrypoint wrappers.
//!
//! Torii, CLI, and SDK boundaries may accept ergonomic JSON, but they convert
//! it before signing into one schema-hashed canonical Norito record. Execution
//! preparation decodes and validates that complete record once. Contract
//! wrappers still supply their compact schema, while the host only verifies the
//! binding and materializes a read-only table of ABI words in declaration order.

use std::{mem::size_of, str::FromStr, sync::Arc};

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    prelude::{AssetDefinitionId, AssetId, DataSpaceId, DomainId, Name, NftId},
};
use iroha_primitives::{json::Json, numeric::Numeric};
use ivm_abi::entrypoint::{
    EntrypointArgumentAtomV1, EntrypointArgumentKindV1, EntrypointArgumentRecordV1,
    EntrypointArgumentSchemaV1, EntrypointArgumentTypeNodeV1, EntrypointArgumentTypeV1,
    EntrypointArgumentWordKindV1, MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES,
    MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES, MAX_ENTRYPOINT_ARGUMENT_WORDS, MAX_ENTRYPOINT_ARGUMENTS,
    entrypoint_argument_schema_hash_v1,
};
use norito::{decode_from_bytes, json as njson, to_bytes};

use crate::{
    VMError,
    ivm::IVM,
    pointer_abi::{self, PointerType, Tlv},
};

const ARGUMENT_DECODE_GAS_BASE: u64 = 32;
const ARGUMENT_DECODE_GAS_PER_BYTE: u64 = 1;

#[cfg(any(test, debug_assertions))]
thread_local! {
    static RECORD_DECODE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Reset the thread-local canonical-record decode counter used by debug tests.
///
/// Release builds keep no counter and this function is a no-op.
#[doc(hidden)]
pub fn reset_argument_record_decode_count() {
    #[cfg(any(test, debug_assertions))]
    RECORD_DECODE_COUNT.with(|count| count.set(0));
}

/// Return the thread-local canonical-record decode count used by debug tests.
///
/// Release builds return zero and incur no decode-path instrumentation.
#[doc(hidden)]
#[must_use]
pub fn argument_record_decode_count() -> usize {
    #[cfg(any(test, debug_assertions))]
    {
        return RECORD_DECODE_COUNT.with(std::cell::Cell::get);
    }
    #[cfg(not(any(test, debug_assertions)))]
    {
        0
    }
}

#[derive(Debug)]
enum DecodedArgument {
    Scalar(u64),
    Pointer(Vec<u8>),
}

#[derive(Debug)]
struct ArgumentDecodePlan {
    decoded: Vec<DecodedArgument>,
    record_bytes: usize,
    schema_bytes: usize,
}

impl ArgumentDecodePlan {
    fn gas(&self) -> u64 {
        argument_record_gas(self.record_bytes, self.schema_bytes)
    }
}

struct PreparedArgumentRecordInner {
    canonical_record: Arc<[u8]>,
    canonical_schema: Arc<[u8]>,
    decode_plan: ArgumentDecodePlan,
}

/// A schema-validated public argument record ready for one IVM invocation.
///
/// The canonical signed bytes and their decoded ABI-word plan are immutable and
/// shared across host construction, access prepasses, and execution. Runtime
/// materialization only assigns VM-local pointers; it never decodes Norito.
#[derive(Clone)]
pub struct PreparedArgumentRecord {
    inner: Arc<PreparedArgumentRecordInner>,
}

impl core::fmt::Debug for PreparedArgumentRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PreparedArgumentRecord")
            .field("record_bytes", &self.inner.canonical_record.len())
            .field("schema_bytes", &self.inner.canonical_schema.len())
            .field("abi_words", &self.inner.decode_plan.decoded.len())
            .finish_non_exhaustive()
    }
}

impl PreparedArgumentRecord {
    /// Return the unchanged canonical record bytes carried by the signed call.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.inner.canonical_record
    }

    /// Return the canonical compiler-emitted schema bytes bound to this record.
    #[must_use]
    pub fn schema_bytes(&self) -> &[u8] {
        &self.inner.canonical_schema
    }

    /// Quote materialization after verifying the VM still presents the exact
    /// record pointer issued by the host and the prepared canonical schema.
    ///
    /// # Errors
    ///
    /// Returns an error if either VM pointer is invalid, substituted, or not
    /// allowed by the active ABI policy.
    pub fn decode_gas_quote(&self, vm: &IVM, record_pointer: u64) -> Result<u64, VMError> {
        self.validate_vm_binding(vm, record_pointer)?;
        Ok(self.inner.decode_plan.gas())
    }

    /// Materialize the prepared ABI-word table in VM input memory.
    ///
    /// This validates the host-issued record pointer and canonical schema again
    /// for direct host calls, then allocates typed pointer values and the packed
    /// word table without decoding either Norito payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM inputs do not match this prepared record or
    /// if VM input memory cannot hold the materialized values and word table.
    pub fn install_into_vm(&self, vm: &mut IVM, record_pointer: u64) -> Result<u64, VMError> {
        self.validate_vm_binding(vm, record_pointer)?;
        materialize_decode_plan(vm, &self.inner.decode_plan)
    }

    fn validate_vm_binding(&self, vm: &IVM, record_pointer: u64) -> Result<(), VMError> {
        if vm.register(10) != record_pointer {
            return Err(VMError::DecodeError);
        }
        let record_tlv = validate_tlv_any_region(vm, record_pointer, PointerType::NoritoBytes)?;
        let schema_tlv = validate_tlv_any_region(vm, vm.register(11), PointerType::NoritoBytes)?;
        validate_argument_envelope_lengths(&record_tlv, &schema_tlv)?;
        if record_tlv.payload != self.canonical_bytes() || schema_tlv.payload != self.schema_bytes()
        {
            return Err(VMError::DecodeError);
        }
        Ok(())
    }
}

fn argument_record_gas(record_bytes: usize, schema_bytes: usize) -> u64 {
    // Typed outputs are embedded in the schema-bound Norito record. Reserve one
    // additional record-sized allowance for their INPUT copies without decoding
    // the record during preparation. V1 charges the maximum table size so the
    // quote depends only on authenticated envelope lengths.
    let table_len =
        1usize.saturating_add(MAX_ENTRYPOINT_ARGUMENT_WORDS.saturating_mul(size_of::<u64>()));
    let charged_bytes = record_bytes
        .saturating_mul(2)
        .saturating_add(schema_bytes)
        .saturating_add(table_len);
    ARGUMENT_DECODE_GAS_BASE.saturating_add(
        ARGUMENT_DECODE_GAS_PER_BYTE
            .saturating_mul(u64::try_from(charged_bytes).unwrap_or(u64::MAX)),
    )
}

fn validate_tlv_any_region(
    vm: &IVM,
    address: u64,
    expected: PointerType,
) -> Result<Tlv<'_>, VMError> {
    let header = vm
        .memory
        .load_region(address, 7)
        .map_err(|_| VMError::NoritoInvalid)?;
    let payload_len = u32::from_be_bytes([header[3], header[4], header[5], header[6]]) as usize;
    let envelope_len = 7usize
        .checked_add(payload_len)
        .and_then(|len| len.checked_add(Hash::LENGTH))
        .ok_or(VMError::NoritoInvalid)?;
    let bytes = vm
        .memory
        .load_region(
            address,
            u64::try_from(envelope_len).map_err(|_| VMError::NoritoInvalid)?,
        )
        .map_err(|_| VMError::NoritoInvalid)?;
    let tlv = pointer_abi::validate_tlv_bytes(bytes)?;
    if tlv.type_id != expected {
        return Err(VMError::NoritoInvalid);
    }
    if !pointer_abi::is_type_allowed_for_policy(vm.syscall_policy(), tlv.type_id) {
        return Err(VMError::AbiTypeNotAllowed {
            abi: vm.abi_version(),
            type_id: tlv.type_id as u16,
        });
    }
    Ok(tlv)
}

fn encode_tlv(pointer_type: PointerType, payload: &[u8]) -> Result<Vec<u8>, VMError> {
    let payload_len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&payload_len.to_be_bytes());
    out.extend_from_slice(payload);
    out.extend_from_slice(Hash::new(payload).as_ref());
    Ok(out)
}

fn decode_i64(value: &njson::Value) -> Result<i64, VMError> {
    match value {
        njson::Value::Number(njson::native::Number::I64(value)) => Ok(*value),
        njson::Value::Number(njson::native::Number::U64(value)) => {
            i64::try_from(*value).map_err(|_| VMError::DecodeError)
        }
        _ => Err(VMError::DecodeError),
    }
}

fn decode_u64(value: &njson::Value) -> Result<u64, VMError> {
    match value {
        njson::Value::Number(number) => number.as_u64().ok_or(VMError::DecodeError),
        _ => Err(VMError::DecodeError),
    }
}

fn decode_u128(value: &njson::Value) -> Result<Numeric, VMError> {
    let raw = value.as_str().ok_or(VMError::DecodeError)?;
    let value = raw.parse::<u128>().map_err(|_| VMError::DecodeError)?;
    if value.to_string() != raw {
        return Err(VMError::DecodeError);
    }
    Ok(Numeric::new(value, 0))
}

fn decode_canonical_string<T>(
    value: &njson::Value,
    parse: impl FnOnce(&str) -> Result<T, VMError>,
    canonical: impl FnOnce(&T) -> String,
) -> Result<T, VMError> {
    let raw = value.as_str().ok_or(VMError::DecodeError)?;
    let parsed = parse(raw)?;
    if canonical(&parsed) != raw {
        return Err(VMError::DecodeError);
    }
    Ok(parsed)
}

fn decode_numeric(value: &njson::Value) -> Result<Numeric, VMError> {
    match value {
        njson::Value::String(raw) => {
            let parsed: Numeric = raw.parse().map_err(|_| VMError::DecodeError)?;
            if parsed.to_string() != *raw {
                return Err(VMError::DecodeError);
            }
            Ok(parsed)
        }
        njson::Value::Number(njson::native::Number::I64(value)) => Ok(Numeric::from(*value)),
        njson::Value::Number(njson::native::Number::U64(value)) => Ok(Numeric::from(*value)),
        _ => Err(VMError::DecodeError),
    }
}

fn decode_blob(value: &njson::Value) -> Result<Vec<u8>, VMError> {
    let raw = value.as_str().ok_or(VMError::DecodeError)?;
    let raw = raw.strip_prefix("0x").ok_or(VMError::DecodeError)?;
    if raw.len() % 2 != 0 {
        return Err(VMError::DecodeError);
    }
    if raw
        .bytes()
        .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(VMError::DecodeError);
    }
    hex::decode(raw).map_err(|_| VMError::DecodeError)
}

fn encode_leaf_atom(
    kind: &EntrypointArgumentKindV1,
    value: &njson::Value,
) -> Result<EntrypointArgumentAtomV1, VMError> {
    let encoded_pointer = |pointer_type, payload: Vec<u8>| {
        encode_tlv(pointer_type, &payload).map(EntrypointArgumentAtomV1::Pointer)
    };
    Ok(match kind {
        EntrypointArgumentKindV1::Int => EntrypointArgumentAtomV1::Int(decode_i64(value)?),
        EntrypointArgumentKindV1::U128 => encoded_pointer(
            PointerType::NoritoBytes,
            to_bytes(&decode_u128(value)?).map_err(|_| VMError::NoritoInvalid)?,
        )?,
        EntrypointArgumentKindV1::Bool => {
            EntrypointArgumentAtomV1::Bool(value.as_bool().ok_or(VMError::DecodeError)?)
        }
        EntrypointArgumentKindV1::String => encoded_pointer(
            PointerType::Blob,
            value
                .as_str()
                .ok_or(VMError::DecodeError)?
                .as_bytes()
                .to_vec(),
        )?,
        EntrypointArgumentKindV1::Numeric => encoded_pointer(
            PointerType::NoritoBytes,
            to_bytes(&decode_numeric(value)?).map_err(|_| VMError::NoritoInvalid)?,
        )?,
        EntrypointArgumentKindV1::Json => encoded_pointer(
            PointerType::Json,
            to_bytes(&Json::from_norito_value_ref(value).map_err(|_| VMError::DecodeError)?)
                .map_err(|_| VMError::NoritoInvalid)?,
        )?,
        EntrypointArgumentKindV1::Name => {
            let value = decode_canonical_string(
                value,
                |raw| Name::from_str(raw).map_err(|_| VMError::DecodeError),
                ToString::to_string,
            )?;
            encoded_pointer(
                PointerType::Name,
                to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointArgumentKindV1::AccountId => {
            let raw = value.as_str().ok_or(VMError::DecodeError)?;
            let parsed = AccountId::parse_encoded(raw).map_err(|_| VMError::DecodeError)?;
            if parsed.canonical() != raw {
                return Err(VMError::DecodeError);
            }
            let value = parsed.into_account_id();
            encoded_pointer(
                PointerType::AccountId,
                to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointArgumentKindV1::AssetDefinitionId => {
            let value = decode_canonical_string(
                value,
                |raw| {
                    AssetDefinitionId::parse_address_literal(raw).map_err(|_| VMError::DecodeError)
                },
                AssetDefinitionId::canonical_address,
            )?;
            encoded_pointer(
                PointerType::AssetDefinitionId,
                to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointArgumentKindV1::AssetId => {
            let value = decode_canonical_string(
                value,
                |raw| AssetId::parse_literal(raw).map_err(|_| VMError::DecodeError),
                AssetId::canonical_literal,
            )?;
            encoded_pointer(
                PointerType::AssetId,
                to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointArgumentKindV1::DomainId => {
            let value = decode_canonical_string(
                value,
                |raw| DomainId::parse_fully_qualified(raw).map_err(|_| VMError::DecodeError),
                ToString::to_string,
            )?;
            encoded_pointer(
                PointerType::DomainId,
                to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointArgumentKindV1::NftId => {
            let value = decode_canonical_string(
                value,
                |raw| NftId::from_str(raw).map_err(|_| VMError::DecodeError),
                ToString::to_string,
            )?;
            encoded_pointer(
                PointerType::NftId,
                to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointArgumentKindV1::DataSpaceId => {
            let value = DataSpaceId::new(decode_u64(value)?);
            encoded_pointer(
                PointerType::DataSpaceId,
                to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointArgumentKindV1::Blob => encoded_pointer(PointerType::Blob, decode_blob(value)?)?,
    })
}

fn append_inactive_node(
    nodes: &[EntrypointArgumentTypeNodeV1],
    node_index: &mut usize,
    out: &mut Vec<EntrypointArgumentAtomV1>,
) -> Result<(), VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        EntrypointArgumentTypeNodeV1::Struct { fields, .. } => {
            for _ in fields {
                append_inactive_node(nodes, node_index, out)?;
            }
        }
        EntrypointArgumentTypeNodeV1::Tuple { arity } => {
            for _ in 0..*arity {
                append_inactive_node(nodes, node_index, out)?;
            }
        }
        EntrypointArgumentTypeNodeV1::Option => {
            out.push(EntrypointArgumentAtomV1::Tag(false));
            append_inactive_node(nodes, node_index, out)?;
        }
        EntrypointArgumentTypeNodeV1::Result => {
            out.push(EntrypointArgumentAtomV1::Tag(false));
            append_inactive_node(nodes, node_index, out)?;
            append_inactive_node(nodes, node_index, out)?;
        }
        EntrypointArgumentTypeNodeV1::Leaf(kind) => {
            out.push(match kind {
                EntrypointArgumentKindV1::Int => EntrypointArgumentAtomV1::Int(0),
                EntrypointArgumentKindV1::Bool => EntrypointArgumentAtomV1::Bool(false),
                _ => EntrypointArgumentAtomV1::Null,
            });
        }
    }
    Ok(())
}

fn decode_argument_node(
    nodes: &[EntrypointArgumentTypeNodeV1],
    node_index: &mut usize,
    value: &njson::Value,
    out: &mut Vec<EntrypointArgumentAtomV1>,
) -> Result<(), VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        EntrypointArgumentTypeNodeV1::Struct { fields, .. } => {
            let object = value.as_object().ok_or(VMError::DecodeError)?;
            if object.len() != fields.len() {
                return Err(VMError::DecodeError);
            }
            for field in fields {
                decode_argument_node(
                    nodes,
                    node_index,
                    object.get(field).ok_or(VMError::DecodeError)?,
                    out,
                )?;
            }
        }
        EntrypointArgumentTypeNodeV1::Tuple { arity } => {
            let values = value.as_array().ok_or(VMError::DecodeError)?;
            if values.len() != usize::from(*arity) {
                return Err(VMError::DecodeError);
            }
            for value in values {
                decode_argument_node(nodes, node_index, value, out)?;
            }
        }
        EntrypointArgumentTypeNodeV1::Option => {
            let object = value.as_object().ok_or(VMError::DecodeError)?;
            if object.len() != 1 {
                return Err(VMError::DecodeError);
            }
            if let Some(value) = object.get("some") {
                out.push(EntrypointArgumentAtomV1::Tag(true));
                decode_argument_node(nodes, node_index, value, out)?;
            } else if object.get("none") == Some(&njson::Value::Bool(true)) {
                out.push(EntrypointArgumentAtomV1::Tag(false));
                append_inactive_node(nodes, node_index, out)?;
            } else {
                return Err(VMError::DecodeError);
            }
        }
        EntrypointArgumentTypeNodeV1::Result => {
            let object = value.as_object().ok_or(VMError::DecodeError)?;
            if object.len() != 1 {
                return Err(VMError::DecodeError);
            }
            if let Some(value) = object.get("ok") {
                out.push(EntrypointArgumentAtomV1::Tag(true));
                decode_argument_node(nodes, node_index, value, out)?;
                append_inactive_node(nodes, node_index, out)?;
            } else if let Some(value) = object.get("err") {
                out.push(EntrypointArgumentAtomV1::Tag(false));
                append_inactive_node(nodes, node_index, out)?;
                decode_argument_node(nodes, node_index, value, out)?;
            } else {
                return Err(VMError::DecodeError);
            }
        }
        EntrypointArgumentTypeNodeV1::Leaf(kind) => out.push(encode_leaf_atom(kind, value)?),
    }
    Ok(())
}

fn decode_argument_value(
    ty: &EntrypointArgumentTypeV1,
    value: &njson::Value,
    out: &mut Vec<EntrypointArgumentAtomV1>,
) -> Result<(), VMError> {
    if !ty.validate() {
        return Err(VMError::DecodeError);
    }
    let mut node_index = 0;
    decode_argument_node(&ty.nodes, &mut node_index, value, out)?;
    if node_index != ty.nodes.len() {
        return Err(VMError::DecodeError);
    }
    Ok(())
}

/// Convert one Torii/CLI boundary JSON value into the canonical schema-bound
/// Norito record consumed by a Kotodama V1 entrypoint.
pub fn argument_record_from_json(
    schema: &EntrypointArgumentSchemaV1,
    payload: &Json,
) -> Result<EntrypointArgumentRecordV1, VMError> {
    if !schema.validate() {
        return Err(VMError::DecodeError);
    }
    let value: njson::Value = payload
        .try_into_any_norito()
        .map_err(|_| VMError::DecodeError)?;
    let object = value.as_object().ok_or(VMError::DecodeError)?;
    if object.len() != schema.fields.len() {
        return Err(VMError::DecodeError);
    }
    let expected_words = schema.word_count().ok_or(VMError::DecodeError)?;
    let mut atoms = Vec::with_capacity(expected_words);
    for field in &schema.fields {
        let value = object.get(&field.name).ok_or(VMError::DecodeError)?;
        decode_argument_value(&field.ty, value, &mut atoms)?;
    }
    if atoms.len() != expected_words || !schema.validate_atoms(&atoms) {
        return Err(VMError::DecodeError);
    }
    let schema_bytes = to_bytes(schema).map_err(|_| VMError::NoritoInvalid)?;
    Ok(EntrypointArgumentRecordV1 {
        schema_hash: entrypoint_argument_schema_hash_v1(&schema_bytes),
        atoms,
    })
}

/// Encode a canonical public argument record for transport into the IVM host.
pub fn encode_argument_record_from_json(
    schema: &EntrypointArgumentSchemaV1,
    payload: &Json,
) -> Result<Vec<u8>, VMError> {
    let record = argument_record_from_json(schema, payload)?;
    let bytes = to_bytes(&record).map_err(|_| VMError::NoritoInvalid)?;
    if bytes.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES {
        return Err(VMError::NoritoInvalid);
    }
    Ok(bytes)
}

fn decode_schema(payload: &[u8]) -> Result<EntrypointArgumentSchemaV1, VMError> {
    if payload.len() > MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES {
        return Err(VMError::DecodeError);
    }
    let schema: EntrypointArgumentSchemaV1 =
        decode_from_bytes(payload).map_err(|_| VMError::DecodeError)?;
    if !schema.validate()
        || schema.fields.len() > MAX_ENTRYPOINT_ARGUMENTS
        || to_bytes(&schema).map_err(|_| VMError::DecodeError)? != payload
    {
        return Err(VMError::DecodeError);
    }
    Ok(schema)
}

fn decode_record(payload: &[u8]) -> Result<EntrypointArgumentRecordV1, VMError> {
    if payload.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES {
        return Err(VMError::DecodeError);
    }
    #[cfg(any(test, debug_assertions))]
    RECORD_DECODE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
    let record: EntrypointArgumentRecordV1 =
        decode_from_bytes(payload).map_err(|_| VMError::DecodeError)?;
    if to_bytes(&record).map_err(|_| VMError::DecodeError)? != payload {
        return Err(VMError::DecodeError);
    }
    Ok(record)
}

fn validate_argument_envelope_lengths(record: &Tlv<'_>, schema: &Tlv<'_>) -> Result<(), VMError> {
    if record.payload.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES
        || schema.payload.len() > MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES
    {
        return Err(VMError::NoritoInvalid);
    }
    Ok(())
}

fn expected_pointer_type(kind: EntrypointArgumentKindV1) -> Option<PointerType> {
    Some(match kind {
        EntrypointArgumentKindV1::Int | EntrypointArgumentKindV1::Bool => return None,
        EntrypointArgumentKindV1::U128 | EntrypointArgumentKindV1::Numeric => {
            PointerType::NoritoBytes
        }
        EntrypointArgumentKindV1::String | EntrypointArgumentKindV1::Blob => PointerType::Blob,
        EntrypointArgumentKindV1::Json => PointerType::Json,
        EntrypointArgumentKindV1::Name => PointerType::Name,
        EntrypointArgumentKindV1::AccountId => PointerType::AccountId,
        EntrypointArgumentKindV1::AssetDefinitionId => PointerType::AssetDefinitionId,
        EntrypointArgumentKindV1::AssetId => PointerType::AssetId,
        EntrypointArgumentKindV1::DomainId => PointerType::DomainId,
        EntrypointArgumentKindV1::NftId => PointerType::NftId,
        EntrypointArgumentKindV1::DataSpaceId => PointerType::DataSpaceId,
    })
}

fn decode_canonical_norito<T>(payload: &[u8]) -> Result<T, VMError>
where
    T: norito::codec::Decode + norito::codec::Encode,
{
    let value = decode_from_bytes(payload).map_err(|_| VMError::DecodeError)?;
    if to_bytes(&value).map_err(|_| VMError::DecodeError)? != payload {
        return Err(VMError::DecodeError);
    }
    Ok(value)
}

fn validate_pointer_payload(kind: EntrypointArgumentKindV1, payload: &[u8]) -> Result<(), VMError> {
    match kind {
        EntrypointArgumentKindV1::Int | EntrypointArgumentKindV1::Bool => {
            return Err(VMError::DecodeError);
        }
        EntrypointArgumentKindV1::U128 => {
            let value: Numeric = decode_canonical_norito(payload)?;
            if value.scale() != 0 || value.try_mantissa_u128().is_none() {
                return Err(VMError::DecodeError);
            }
        }
        EntrypointArgumentKindV1::Numeric => {
            let _: Numeric = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::String => {
            std::str::from_utf8(payload).map_err(|_| VMError::DecodeError)?;
        }
        EntrypointArgumentKindV1::Json => {
            let _: Json = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::Name => {
            let _: Name = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::AccountId => {
            let _: AccountId = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::AssetDefinitionId => {
            let _: AssetDefinitionId = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::AssetId => {
            let _: AssetId = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::DomainId => {
            let _: DomainId = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::NftId => {
            let _: NftId = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::DataSpaceId => {
            let _: DataSpaceId = decode_canonical_norito(payload)?;
        }
        EntrypointArgumentKindV1::Blob => {}
    }
    Ok(())
}

fn validate_pointer_atom(
    policy: ivm_abi::SyscallPolicy,
    kind: EntrypointArgumentKindV1,
    envelope: &[u8],
) -> Result<(), VMError> {
    let expected = expected_pointer_type(kind).ok_or(VMError::DecodeError)?;
    let tlv = pointer_abi::validate_tlv_bytes(envelope)?;
    if tlv.type_id != expected
        || !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id)
        || encode_tlv(tlv.type_id, tlv.payload)?.as_slice() != envelope
    {
        return Err(VMError::DecodeError);
    }
    validate_pointer_payload(kind, tlv.payload)
}

fn validate_record_shape(
    schema: &EntrypointArgumentSchemaV1,
    schema_bytes: &[u8],
    record: &EntrypointArgumentRecordV1,
    policy: ivm_abi::SyscallPolicy,
) -> Result<Vec<EntrypointArgumentWordKindV1>, VMError> {
    if record.schema_hash != entrypoint_argument_schema_hash_v1(schema_bytes)
        || !schema.validate_atoms(&record.atoms)
    {
        return Err(VMError::DecodeError);
    }
    let word_kinds = schema.word_kinds().ok_or(VMError::DecodeError)?;
    if word_kinds.len() != record.atoms.len() {
        return Err(VMError::DecodeError);
    }
    for (kind, atom) in word_kinds.iter().copied().zip(&record.atoms) {
        match (kind, atom) {
            (EntrypointArgumentWordKindV1::Tag, EntrypointArgumentAtomV1::Tag(_))
            | (
                EntrypointArgumentWordKindV1::Leaf(EntrypointArgumentKindV1::Int),
                EntrypointArgumentAtomV1::Int(_),
            )
            | (
                EntrypointArgumentWordKindV1::Leaf(EntrypointArgumentKindV1::Bool),
                EntrypointArgumentAtomV1::Bool(_),
            ) => {}
            (EntrypointArgumentWordKindV1::Leaf(kind), EntrypointArgumentAtomV1::Null)
                if kind.is_pointer() => {}
            (
                EntrypointArgumentWordKindV1::Leaf(kind),
                EntrypointArgumentAtomV1::Pointer(envelope),
            ) if kind.is_pointer() => validate_pointer_atom(policy, kind, envelope)?,
            _ => return Err(VMError::DecodeError),
        }
    }
    Ok(word_kinds)
}

/// Validate canonical record bytes against an exact compiler-emitted schema.
///
/// The byte bound is checked before Norito decoding. Canonical re-encoding,
/// schema binding, atom shape, inactive payloads, pointer envelopes, and typed
/// pointer payloads are all fail-closed.
pub fn validate_argument_record(
    schema: &EntrypointArgumentSchemaV1,
    payload: &[u8],
) -> Result<EntrypointArgumentRecordV1, VMError> {
    if !schema.validate() || payload.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES {
        return Err(VMError::DecodeError);
    }
    let record = decode_record(payload)?;
    let schema_bytes = to_bytes(schema).map_err(|_| VMError::DecodeError)?;
    validate_record_shape(
        schema,
        &schema_bytes,
        &record,
        ivm_abi::SyscallPolicy::AbiV1,
    )?;
    Ok(record)
}

fn build_decode_plan(
    schema: &EntrypointArgumentSchemaV1,
    schema_bytes: &[u8],
    record: EntrypointArgumentRecordV1,
    policy: ivm_abi::SyscallPolicy,
    record_bytes: usize,
) -> Result<ArgumentDecodePlan, VMError> {
    let word_kinds = validate_record_shape(schema, schema_bytes, &record, policy)?;
    let mut decoded = Vec::with_capacity(word_kinds.len());
    for (kind, atom) in word_kinds.into_iter().zip(record.atoms) {
        let value = match (kind, atom) {
            (EntrypointArgumentWordKindV1::Tag, EntrypointArgumentAtomV1::Tag(value)) => {
                DecodedArgument::Scalar(u64::from(value))
            }
            (
                EntrypointArgumentWordKindV1::Leaf(EntrypointArgumentKindV1::Int),
                EntrypointArgumentAtomV1::Int(value),
            ) => DecodedArgument::Scalar(value as u64),
            (
                EntrypointArgumentWordKindV1::Leaf(EntrypointArgumentKindV1::Bool),
                EntrypointArgumentAtomV1::Bool(value),
            ) => DecodedArgument::Scalar(u64::from(value)),
            (EntrypointArgumentWordKindV1::Leaf(kind), EntrypointArgumentAtomV1::Null)
                if kind.is_pointer() =>
            {
                DecodedArgument::Scalar(0)
            }
            (
                EntrypointArgumentWordKindV1::Leaf(kind),
                EntrypointArgumentAtomV1::Pointer(envelope),
            ) if kind.is_pointer() => DecodedArgument::Pointer(envelope),
            _ => return Err(VMError::DecodeError),
        };
        decoded.push(value);
    }

    Ok(ArgumentDecodePlan {
        decoded,
        record_bytes,
        schema_bytes: schema_bytes.len(),
    })
}

/// Validate and decode a canonical public argument record into a reusable ABI
/// word plan.
///
/// `canonical_record` remains byte-for-byte identical to the signed payload.
/// The only canonical Norito record decode occurs here, before a VM starts.
///
/// # Errors
///
/// Returns an error for an invalid schema, non-canonical record, schema-hash or
/// atom mismatch, disallowed pointer type, or non-canonical typed payload.
pub fn prepare_argument_record(
    schema: &EntrypointArgumentSchemaV1,
    canonical_record: Arc<[u8]>,
) -> Result<PreparedArgumentRecord, VMError> {
    if !schema.validate() || canonical_record.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES {
        return Err(VMError::DecodeError);
    }
    let schema_bytes: Arc<[u8]> = Arc::from(to_bytes(schema).map_err(|_| VMError::DecodeError)?);
    if schema_bytes.len() > MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES {
        return Err(VMError::DecodeError);
    }
    let record = decode_record(&canonical_record)?;
    let decode_plan = build_decode_plan(
        schema,
        &schema_bytes,
        record,
        ivm_abi::SyscallPolicy::AbiV1,
        canonical_record.len(),
    )?;
    Ok(PreparedArgumentRecord {
        inner: Arc::new(PreparedArgumentRecordInner {
            canonical_record,
            canonical_schema: schema_bytes,
            decode_plan,
        }),
    })
}

fn plan_argument_record_decode(vm: &IVM) -> Result<ArgumentDecodePlan, VMError> {
    let record_tlv = validate_tlv_any_region(vm, vm.register(10), PointerType::NoritoBytes)?;
    let schema_tlv = validate_tlv_any_region(vm, vm.register(11), PointerType::NoritoBytes)?;
    validate_argument_envelope_lengths(&record_tlv, &schema_tlv)?;
    let schema = decode_schema(schema_tlv.payload)?;
    let record_bytes = record_tlv.payload.len();

    let record = decode_record(record_tlv.payload)?;
    build_decode_plan(
        &schema,
        schema_tlv.payload,
        record,
        vm.syscall_policy(),
        record_bytes,
    )
}

/// Compute the exact gas charged by [`decode_argument_record`] without
/// decoding either payload, allocating, or changing VM state.
pub(crate) fn decode_argument_record_gas_quote(vm: &IVM) -> Result<u64, VMError> {
    let record_tlv = validate_tlv_any_region(vm, vm.register(10), PointerType::NoritoBytes)?;
    let schema_tlv = validate_tlv_any_region(vm, vm.register(11), PointerType::NoritoBytes)?;
    validate_argument_envelope_lengths(&record_tlv, &schema_tlv)?;
    Ok(argument_record_gas(
        record_tlv.payload.len(),
        schema_tlv.payload.len(),
    ))
}

/// Decode the public argument object in `r10` using the schema in `r11`.
///
/// On success, `r10` receives a `Blob` pointer whose payload is a packed
/// little-endian table of `u64` ABI words after one canonical alignment byte.
/// The operation decodes the boundary payload exactly once, before allocating
/// any typed result envelopes.
pub(crate) fn decode_argument_record(vm: &mut IVM) -> Result<u64, VMError> {
    let plan = plan_argument_record_decode(vm)?;
    materialize_decode_plan(vm, &plan)
}

fn materialize_decode_plan(vm: &mut IVM, plan: &ArgumentDecodePlan) -> Result<u64, VMError> {
    let gas = plan.gas();
    let mut words = Vec::with_capacity(plan.decoded.len());
    for value in &plan.decoded {
        let word = match value {
            DecodedArgument::Scalar(value) => *value,
            DecodedArgument::Pointer(envelope) => vm.alloc_input_tlv(envelope)?,
        };
        words.push(word);
    }

    let mut table = Vec::with_capacity(1 + words.len() * core::mem::size_of::<u64>());
    table.push(0);
    for word in words {
        table.extend_from_slice(&word.to_le_bytes());
    }
    let table_pointer = vm.alloc_input_tlv(&encode_tlv(PointerType::Blob, &table)?)?;
    vm.set_register(10, table_pointer);
    Ok(gas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ivm_abi::entrypoint::{
        EntrypointArgumentFieldV1, EntrypointArgumentKindV1, EntrypointArgumentTypeNodeV1,
        EntrypointArgumentTypeV1,
    };

    fn argument_type(kind: EntrypointArgumentKindV1) -> EntrypointArgumentTypeV1 {
        EntrypointArgumentTypeV1 {
            nodes: vec![EntrypointArgumentTypeNodeV1::Leaf(kind)],
        }
    }

    fn alloc(vm: &mut IVM, pointer_type: PointerType, payload: &[u8]) -> u64 {
        vm.alloc_input_tlv(&encode_tlv(pointer_type, payload).expect("encode TLV"))
            .expect("allocate TLV")
    }

    fn install_record(schema: &EntrypointArgumentSchemaV1, payload: &Json) -> IVM {
        let mut vm = IVM::new(u64::MAX);
        let record = encode_argument_record_from_json(schema, payload)
            .expect("convert boundary JSON to argument record");
        let record_ptr = alloc(&mut vm, PointerType::NoritoBytes, &record);
        let schema_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            &to_bytes(schema).expect("encode schema"),
        );
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        vm
    }

    fn install_raw_record(
        schema: &EntrypointArgumentSchemaV1,
        record: &EntrypointArgumentRecordV1,
    ) -> IVM {
        let mut vm = IVM::new(u64::MAX);
        let record_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            &to_bytes(record).expect("encode raw argument record"),
        );
        let schema_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            &to_bytes(schema).expect("encode raw argument schema"),
        );
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        vm
    }

    fn decoded_words(vm: &IVM) -> Vec<u64> {
        let table = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("decoded argument table");
        assert_eq!(table.type_id, PointerType::Blob);
        assert_eq!(table.payload.first(), Some(&0));
        table.payload[1..]
            .chunks_exact(size_of::<u64>())
            .map(|word| u64::from_le_bytes(word.try_into().expect("argument word")))
            .collect()
    }

    #[test]
    fn complete_record_performs_exactly_one_record_decode() {
        RECORD_DECODE_COUNT.with(|count| count.set(0));
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "count".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "label".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Name),
                },
                EntrypointArgumentFieldV1 {
                    name: "bytes".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Blob),
                },
            ],
        };
        let payload = Json::from(norito::json!({
            "count": 7,
            "label": "ready",
            "bytes": "0x0102",
        }));
        let mut vm = install_record(&schema, &payload);

        decode_argument_record(&mut vm).expect("decode complete record");

        RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 1));
        let table = vm
            .memory
            .validate_tlv(vm.register(10))
            .expect("result table TLV");
        assert_eq!(table.type_id, PointerType::Blob);
        assert_eq!(table.payload.len(), 1 + 3 * core::mem::size_of::<u64>());
        assert_eq!(table.payload[0], 0, "alignment prefix must be canonical");
        let count = u64::from_le_bytes(table.payload[1..9].try_into().expect("count word"));
        assert_eq!(count, 7);
        for bytes in [9..17, 17..25] {
            let pointer =
                u64::from_le_bytes(table.payload[bytes].try_into().expect("pointer word"));
            vm.memory.validate_tlv(pointer).expect("typed output TLV");
        }
    }

    #[test]
    fn prepared_record_materializes_without_a_second_record_decode() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "count".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "label".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Name),
                },
            ],
        };
        let payload = Json::from(norito::json!({
            "count": 7,
            "label": "ready",
        }));
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(&schema, &payload).expect("encode argument record"),
        );
        RECORD_DECODE_COUNT.with(|count| count.set(0));
        let prepared =
            prepare_argument_record(&schema, Arc::clone(&canonical)).expect("prepare arguments");
        let shared = prepared.clone();
        assert!(core::ptr::eq(
            prepared.canonical_bytes().as_ptr(),
            shared.canonical_bytes().as_ptr(),
        ));

        let mut vm = IVM::new(u64::MAX);
        let record_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            prepared.canonical_bytes(),
        );
        let schema_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.schema_bytes());
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        prepared
            .decode_gas_quote(&vm, record_ptr)
            .expect("quote prepared arguments");
        prepared
            .install_into_vm(&mut vm, record_ptr)
            .expect("materialize prepared arguments");

        RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 1));
        assert_eq!(decoded_words(&vm)[0], 7);
    }

    #[test]
    fn prepared_record_rejects_substituted_record_or_schema() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "count".to_owned(),
                ty: argument_type(EntrypointArgumentKindV1::Int),
            }],
        };
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(&schema, &Json::from(norito::json!({"count": 7})))
                .expect("encode argument record"),
        );
        let prepared =
            prepare_argument_record(&schema, Arc::clone(&canonical)).expect("prepare arguments");
        let mut vm = IVM::new(u64::MAX);
        let issued_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            prepared.canonical_bytes(),
        );
        let substituted_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            prepared.canonical_bytes(),
        );
        let schema_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.schema_bytes());
        vm.set_register(10, substituted_ptr);
        vm.set_register(11, schema_ptr);
        assert!(matches!(
            prepared.decode_gas_quote(&vm, issued_ptr),
            Err(VMError::DecodeError)
        ));

        let other_schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "different".to_owned(),
                ty: argument_type(EntrypointArgumentKindV1::Int),
            }],
        };
        let other_schema_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            &to_bytes(&other_schema).expect("encode alternate schema"),
        );
        vm.set_register(10, issued_ptr);
        vm.set_register(11, other_schema_ptr);
        assert!(matches!(
            prepared.decode_gas_quote(&vm, issued_ptr),
            Err(VMError::DecodeError)
        ));
    }

    #[test]
    fn canonical_v1_scalars_and_typed_ids_roundtrip() {
        let account = AccountId::new(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
                .parse()
                .expect("fixture public key"),
        );
        let mut definition_bytes = [0x11; 16];
        definition_bytes[6] = 0x41;
        definition_bytes[8] = 0x81;
        let definition = AssetDefinitionId::from_uuid_bytes(definition_bytes)
            .expect("fixture UUIDv4 asset definition");
        let asset = AssetId::new(definition.clone(), account.clone());
        let domain = DomainId::try_new("wonderland", "universal").expect("fixture domain");
        let nft = NftId::new(
            domain.clone(),
            "collectible".parse().expect("fixture NFT name"),
        );
        let dataspace = DataSpaceId::new(u64::MAX);
        let wide = u128::MAX;
        let account_literal = account.canonical_i105().expect("canonical account literal");
        let definition_literal = definition.canonical_address();
        let asset_literal = asset.canonical_literal();
        let domain_literal = domain.to_string();
        let nft_literal = nft.to_string();
        let wide_literal = wide.to_string();
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "ready".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Bool),
                },
                EntrypointArgumentFieldV1 {
                    name: "memo".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::String),
                },
                EntrypointArgumentFieldV1 {
                    name: "wide".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::U128),
                },
                EntrypointArgumentFieldV1 {
                    name: "account".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::AccountId),
                },
                EntrypointArgumentFieldV1 {
                    name: "definition".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::AssetDefinitionId),
                },
                EntrypointArgumentFieldV1 {
                    name: "asset".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::AssetId),
                },
                EntrypointArgumentFieldV1 {
                    name: "domain".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::DomainId),
                },
                EntrypointArgumentFieldV1 {
                    name: "nft".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::NftId),
                },
                EntrypointArgumentFieldV1 {
                    name: "dataspace".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::DataSpaceId),
                },
            ],
        };
        let payload = Json::from(norito::json!({
            "ready": true,
            "memo": "言霊",
            "wide": wide_literal,
            "account": account_literal,
            "definition": definition_literal,
            "asset": asset_literal,
            "domain": domain_literal,
            "nft": nft_literal,
            "dataspace": (dataspace.as_u64()),
        }));
        let mut vm = install_record(&schema, &payload);

        decode_argument_record(&mut vm).expect("decode canonical V1 arguments");
        let words = decoded_words(&vm);
        assert_eq!(words.len(), schema.fields.len());
        assert_eq!(words[0], 1);

        let string = vm.memory.validate_tlv(words[1]).expect("string TLV");
        assert_eq!(string.type_id, PointerType::Blob);
        assert_eq!(string.payload, "言霊".as_bytes());

        let wide_tlv = vm.memory.validate_tlv(words[2]).expect("u128 TLV");
        assert_eq!(wide_tlv.type_id, PointerType::NoritoBytes);
        let decoded_wide: Numeric =
            decode_from_bytes(wide_tlv.payload).expect("decode scale-zero Numeric");
        assert_eq!(decoded_wide.to_string(), wide.to_string());

        let account_tlv = vm.memory.validate_tlv(words[3]).expect("AccountId TLV");
        assert_eq!(account_tlv.type_id, PointerType::AccountId);
        assert_eq!(
            decode_from_bytes::<AccountId>(account_tlv.payload).expect("decode AccountId"),
            account
        );
        let definition_tlv = vm
            .memory
            .validate_tlv(words[4])
            .expect("AssetDefinitionId TLV");
        assert_eq!(definition_tlv.type_id, PointerType::AssetDefinitionId);
        assert_eq!(
            decode_from_bytes::<AssetDefinitionId>(definition_tlv.payload)
                .expect("decode AssetDefinitionId"),
            definition
        );
        let asset_tlv = vm.memory.validate_tlv(words[5]).expect("AssetId TLV");
        assert_eq!(asset_tlv.type_id, PointerType::AssetId);
        assert_eq!(
            decode_from_bytes::<AssetId>(asset_tlv.payload).expect("decode AssetId"),
            asset
        );
        let domain_tlv = vm.memory.validate_tlv(words[6]).expect("DomainId TLV");
        assert_eq!(domain_tlv.type_id, PointerType::DomainId);
        assert_eq!(
            decode_from_bytes::<DomainId>(domain_tlv.payload).expect("decode DomainId"),
            domain
        );
        let nft_tlv = vm.memory.validate_tlv(words[7]).expect("NftId TLV");
        assert_eq!(nft_tlv.type_id, PointerType::NftId);
        assert_eq!(
            decode_from_bytes::<NftId>(nft_tlv.payload).expect("decode NftId"),
            nft
        );
        let dataspace_tlv = vm.memory.validate_tlv(words[8]).expect("DataSpaceId TLV");
        assert_eq!(dataspace_tlv.type_id, PointerType::DataSpaceId);
        assert_eq!(
            decode_from_bytes::<DataSpaceId>(dataspace_tlv.payload).expect("decode DataSpaceId"),
            dataspace
        );
    }

    #[test]
    fn recursive_struct_tuple_option_and_result_decode_in_one_pass() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "request".into(),
                ty: EntrypointArgumentTypeV1 {
                    nodes: vec![
                        EntrypointArgumentTypeNodeV1::Struct {
                            name: "Request".into(),
                            fields: vec!["pair".into(), "memo".into(), "outcome".into()],
                        },
                        EntrypointArgumentTypeNodeV1::Tuple { arity: 2 },
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Int),
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Bool),
                        EntrypointArgumentTypeNodeV1::Option,
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::String),
                        EntrypointArgumentTypeNodeV1::Result,
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Name),
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Bool),
                    ],
                },
            }],
        };
        assert_eq!(schema.word_count(), Some(7));
        let payload = Json::from(norito::json!({
            "request": {
                "pair": [7, true],
                "memo": { "some": "言霊" },
                "outcome": { "err": true },
            },
        }));
        RECORD_DECODE_COUNT.with(|count| count.set(0));
        let mut vm = install_record(&schema, &payload);

        decode_argument_record(&mut vm).expect("decode recursive argument");
        RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 1));
        let words = decoded_words(&vm);
        assert_eq!(words.len(), 7);
        assert_eq!(words[0], 7);
        assert_eq!(words[1], 1);
        assert_eq!(words[2], 1, "Some tag");
        let memo = vm.memory.validate_tlv(words[3]).expect("memo string TLV");
        assert_eq!(memo.type_id, PointerType::Blob);
        assert_eq!(memo.payload, "言霊".as_bytes());
        assert_eq!(words[4], 0, "Err tag");
        assert_eq!(words[5], 0, "inactive Name payload must be null");
        assert_eq!(words[6], 1);
    }

    #[test]
    fn recursive_tags_reject_ambiguous_or_noncanonical_shapes() {
        let option_type = EntrypointArgumentTypeV1 {
            nodes: vec![
                EntrypointArgumentTypeNodeV1::Option,
                EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Int),
            ],
        };
        let invalid = [
            norito::json!({ "some": 1, "none": true }),
            norito::json!({ "none": false }),
            norito::json!({ "None": true }),
            njson::Value::Null,
        ];
        for value in invalid {
            let schema = EntrypointArgumentSchemaV1 {
                fields: vec![EntrypointArgumentFieldV1 {
                    name: "value".into(),
                    ty: option_type.clone(),
                }],
            };
            let payload = Json::from(norito::json!({ "value": value }));
            assert_eq!(
                argument_record_from_json(&schema, &payload),
                Err(VMError::DecodeError)
            );
        }
    }

    #[test]
    fn runtime_rejects_schema_mismatch_hidden_payload_and_malformed_pointer() {
        let option_schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".into(),
                ty: EntrypointArgumentTypeV1 {
                    nodes: vec![
                        EntrypointArgumentTypeNodeV1::Option,
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Int),
                    ],
                },
            }],
        };
        let option_schema_bytes = to_bytes(&option_schema).expect("option schema bytes");
        let hidden = EntrypointArgumentRecordV1 {
            schema_hash: entrypoint_argument_schema_hash_v1(&option_schema_bytes),
            atoms: vec![
                EntrypointArgumentAtomV1::Tag(false),
                EntrypointArgumentAtomV1::Int(99),
            ],
        };
        let mut vm = install_raw_record(&option_schema, &hidden);
        assert_eq!(decode_argument_record(&mut vm), Err(VMError::DecodeError));

        let wrong_hash = EntrypointArgumentRecordV1 {
            schema_hash: [7; 32],
            atoms: vec![
                EntrypointArgumentAtomV1::Tag(false),
                EntrypointArgumentAtomV1::Int(0),
            ],
        };
        let mut vm = install_raw_record(&option_schema, &wrong_hash);
        assert_eq!(decode_argument_record(&mut vm), Err(VMError::DecodeError));

        let name_schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".into(),
                ty: argument_type(EntrypointArgumentKindV1::Name),
            }],
        };
        let name_schema_bytes = to_bytes(&name_schema).expect("Name schema bytes");
        let malformed_name = EntrypointArgumentRecordV1 {
            schema_hash: entrypoint_argument_schema_hash_v1(&name_schema_bytes),
            atoms: vec![EntrypointArgumentAtomV1::Pointer(
                encode_tlv(PointerType::Name, b"hash-valid but not Norito Name")
                    .expect("malformed Name envelope"),
            )],
        };
        let mut vm = install_raw_record(&name_schema, &malformed_name);
        assert_eq!(decode_argument_record(&mut vm), Err(VMError::DecodeError));
    }

    #[test]
    fn canonical_record_rejects_trailing_truncated_and_adversarial_counts() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".into(),
                ty: argument_type(EntrypointArgumentKindV1::Int),
            }],
        };
        let canonical =
            encode_argument_record_from_json(&schema, &Json::from(norito::json!({ "value": 7 })))
                .expect("canonical record");
        validate_argument_record(&schema, &canonical).expect("valid record");

        let mut trailing = canonical.clone();
        trailing.push(0);
        assert_eq!(
            validate_argument_record(&schema, &trailing),
            Err(VMError::DecodeError)
        );
        for end in 0..canonical.len() {
            assert_eq!(
                validate_argument_record(&schema, &canonical[..end]),
                Err(VMError::DecodeError),
                "truncation at byte {end} must fail closed"
            );
        }

        let empty = EntrypointArgumentRecordV1 {
            schema_hash: [0xa5; 32],
            atoms: Vec::new(),
        };
        let mut forged_count = to_bytes(&empty).expect("encode empty record");
        let count_offset = forged_count
            .windows(size_of::<u64>())
            .rposition(|window| window == [0; size_of::<u64>()])
            .expect("empty atom sequence count");
        forged_count[count_offset..count_offset + size_of::<u64>()]
            .copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            validate_argument_record(&schema, &forged_count),
            Err(VMError::DecodeError),
            "adversarial atom counts must not allocate or panic"
        );
    }

    #[test]
    fn new_v1_kinds_reject_noncanonical_types_after_length_only_quote() {
        let cases = [
            (
                EntrypointArgumentKindV1::Bool,
                njson::Value::String("true".to_owned()),
            ),
            (EntrypointArgumentKindV1::String, njson::Value::Bool(true)),
            (EntrypointArgumentKindV1::U128, njson::Value::from(7_u64)),
            (
                EntrypointArgumentKindV1::U128,
                njson::Value::String("01".to_owned()),
            ),
            (
                EntrypointArgumentKindV1::AssetId,
                njson::Value::String("not-an-asset".to_owned()),
            ),
            (
                EntrypointArgumentKindV1::DomainId,
                njson::Value::String("missing_dataspace".to_owned()),
            ),
            (
                EntrypointArgumentKindV1::DataSpaceId,
                njson::Value::String("7".to_owned()),
            ),
            (
                EntrypointArgumentKindV1::Blob,
                njson::Value::String("0102".to_owned()),
            ),
            (
                EntrypointArgumentKindV1::Blob,
                njson::Value::String("0xAB".to_owned()),
            ),
            (
                EntrypointArgumentKindV1::Blob,
                njson::Value::String("hash:0102".to_owned()),
            ),
        ];

        for (kind, value) in cases {
            RECORD_DECODE_COUNT.with(|count| count.set(0));
            let schema = EntrypointArgumentSchemaV1 {
                fields: vec![EntrypointArgumentFieldV1 {
                    name: "value".into(),
                    ty: argument_type(kind),
                }],
            };
            let payload = Json::from(norito::json!({ "value": value }));
            assert_eq!(
                argument_record_from_json(&schema, &payload),
                Err(VMError::DecodeError)
            );
            RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 0));
        }
    }

    fn quote_fixture() -> IVM {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "count".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "bytes".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Blob),
                },
            ],
        };
        let max_count = i64::MAX;
        let payload = Json::from(norito::json!({
            "count": max_count,
            "bytes": "0x000102ff",
        }));
        install_record(&schema, &payload)
    }

    #[test]
    fn gas_quote_is_exact_repeatable_and_side_effect_free() {
        RECORD_DECODE_COUNT.with(|count| count.set(0));
        let mut quoted_vm = quote_fixture();
        let mut control_vm = quote_fixture();
        let input_registers = (quoted_vm.register(10), quoted_vm.register(11));

        let quote = decode_argument_record_gas_quote(&quoted_vm).expect("quote valid record");
        assert_eq!(
            decode_argument_record_gas_quote(&quoted_vm).expect("repeat quote"),
            quote
        );
        assert_eq!(
            (quoted_vm.register(10), quoted_vm.register(11)),
            input_registers
        );
        RECORD_DECODE_COUNT.with(|count| {
            assert_eq!(
                count.get(),
                0,
                "prepare/quote must not deserialize the argument record"
            );
        });

        let sentinel = encode_tlv(PointerType::Blob, b"sentinel").expect("encode sentinel");
        let quoted_next = quoted_vm
            .alloc_input_tlv(&sentinel)
            .expect("allocate after quote");
        let control_next = control_vm
            .alloc_input_tlv(&sentinel)
            .expect("allocate without quote");
        assert_eq!(quoted_next, control_next, "quote must not advance INPUT");

        assert_eq!(
            decode_argument_record(&mut quoted_vm).expect("execute valid record"),
            quote
        );
        RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 1));
    }

    #[test]
    fn unaffordable_vm_dispatch_never_decodes_or_allocates_outputs() {
        RECORD_DECODE_COUNT.with(|count| count.set(0));
        let mut vm = quote_fixture();
        let quote = decode_argument_record_gas_quote(&vm).expect("quote argument record");
        let input_registers = (vm.register(10), vm.register(11));
        let sentinel = encode_tlv(PointerType::Blob, b"sentinel").expect("encode sentinel");
        let control_next = vm.alloc_input_tlv(&sentinel).expect("control allocation");

        let mut program = crate::metadata::ProgramMetadata::default().encode();
        program.extend_from_slice(
            &crate::encoding::wide::encode_syscallx(
                ivm_abi::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD,
            )
            .to_le_bytes(),
        );
        program.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
        vm.load_program(&program).expect("load argument syscall");
        vm.set_register(10, input_registers.0);
        vm.set_register(11, input_registers.1);
        vm.set_host(crate::host::DefaultHost::new());
        vm.set_gas_limit(5_u64.saturating_add(quote).saturating_sub(1));

        assert_eq!(vm.run(), Err(VMError::OutOfGas));
        RECORD_DECODE_COUNT
            .with(|count| assert_eq!(count.get(), 0, "unaffordable calls must not deserialize"));
        assert_eq!((vm.register(10), vm.register(11)), input_registers);
        let next = vm
            .alloc_input_tlv(&sentinel)
            .expect("allocation after rejected call");
        let expected_next = crate::memory::Memory::INPUT_START
            + ((control_next - crate::memory::Memory::INPUT_START
                + u64::try_from(sentinel.len()).expect("sentinel length fits u64")
                + 7)
                & !7);
        assert_eq!(
            next, expected_next,
            "rejected dispatch must not allocate typed outputs"
        );
    }

    #[test]
    fn gas_quote_does_not_decode_invalid_schema_before_debit() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "same".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "same".to_owned(),
                    ty: argument_type(EntrypointArgumentKindV1::Blob),
                },
            ],
        };
        let record = EntrypointArgumentRecordV1 {
            schema_hash: [0; 32],
            atoms: Vec::new(),
        };
        let mut vm = IVM::new(u64::MAX);
        let record_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            &to_bytes(&record).expect("encode record"),
        );
        let schema_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            &to_bytes(&schema).expect("encode schema"),
        );
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        let before = (vm.register(10), vm.register(11));

        let quote = decode_argument_record_gas_quote(&vm)
            .expect("authenticated envelope lengths are sufficient to quote");
        assert!(quote > 0);
        assert_eq!((vm.register(10), vm.register(11)), before);
        assert_eq!(
            decode_argument_record(&mut vm),
            Err(VMError::DecodeError),
            "schema validation belongs to post-debit execution"
        );
    }

    #[test]
    fn gas_quote_arithmetic_saturates_instead_of_wrapping() {
        let plan = ArgumentDecodePlan {
            decoded: Vec::new(),
            record_bytes: usize::MAX,
            schema_bytes: usize::MAX,
        };
        let expected = ARGUMENT_DECODE_GAS_BASE.saturating_add(
            ARGUMENT_DECODE_GAS_PER_BYTE
                .saturating_mul(u64::try_from(usize::MAX).unwrap_or(u64::MAX)),
        );
        assert_eq!(plan.gas(), expected);
    }

    #[test]
    fn quote_rejects_oversized_envelopes_without_decoding_them() {
        RECORD_DECODE_COUNT.with(|count| count.set(0));
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".into(),
                ty: argument_type(EntrypointArgumentKindV1::Int),
            }],
        };
        let mut vm = IVM::new(u64::MAX);
        let oversized = vec![0_u8; MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES + 1];
        let record_ptr = alloc(&mut vm, PointerType::NoritoBytes, &oversized);
        let schema_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            &to_bytes(&schema).expect("encode schema"),
        );
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);

        assert_eq!(
            decode_argument_record_gas_quote(&vm),
            Err(VMError::NoritoInvalid)
        );
        RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 0));
    }
}
