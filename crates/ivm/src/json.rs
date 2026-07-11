//! Native, schema-bound Kotodama JSON construction and typed getters.
//!
//! Signed `i64` values use JSON numbers. `Amount` and `u128` values that do not
//! fit Norito JSON's integer-number representation use canonical decimal
//! strings, preserving exactness without floating-point conversion.

use core::str::FromStr;

use iroha_crypto::Hash;
use iroha_data_model::{
    account::{AccountId, ParsedAccountId},
    prelude::{AssetDefinitionId, AssetId, DataSpaceId, DomainId, Name, NftId},
};
use iroha_primitives::{json::Json, numeric::Numeric};
use ivm_abi::{
    json::{
        JsonConstructionNodeV1, JsonConstructionSchemaV1, MAX_JSON_CONSTRUCTION_SCHEMA_BYTES_V1,
    },
    state_value::{MAX_STATE_VALUE_WORDS, StateValueKindV1, StateValueNodeV1, StateValueSchemaV1},
};
use norito::{
    NoritoDeserialize, core::NoritoSerialize, decode_from_bytes, json as njson, to_bytes,
};

use crate::{
    IVM, PointerType, VMError, host::preflight_reserved_syscall_gas, pointer_abi, syscalls,
};

/// Base gas for a schema-bound native JSON construction.
pub const JSON_BUILD_GAS_BASE: u64 = 32;

/// Base gas for one typed JSON getter.
pub const JSON_TYPED_GETTER_GAS_BASE: u64 = 16;

/// Address translation used for compiler literal TLVs.
pub type AddressResolver = fn(&IVM, u64) -> u64;

/// Result metadata for one typed getter execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonGetterCost {
    /// Canonical input payload bytes inspected.
    pub input_bytes: usize,
    /// Typed payload and Option allocation bytes materialized.
    pub output_bytes: usize,
}

#[derive(Default)]
struct BuildStats {
    source_bytes: usize,
    collection_elements: usize,
}

fn load_tlv<'a>(
    vm: &'a IVM,
    address: u64,
    expected: PointerType,
    resolver: AddressResolver,
) -> Result<pointer_abi::Tlv<'a>, VMError> {
    let resolved = resolver(vm, address);
    let tlv = vm.validate_tlv(resolved)?;
    if tlv.type_id != expected
        || !pointer_abi::is_type_allowed_for_policy(vm.syscall_policy(), tlv.type_id)
    {
        if crate::dev_env::decode_trace_enabled() {
            eprintln!(
                "[json] pointer type mismatch: raw=0x{address:x} resolved=0x{resolved:x} expected={expected:?} actual={:?}",
                tlv.type_id
            );
        }
        return Err(VMError::NoritoInvalid);
    }
    Ok(tlv)
}

fn load_getter_tlv<'a>(
    vm: &'a IVM,
    address: u64,
    expected: PointerType,
    direct: bool,
    resolver: AddressResolver,
) -> Result<pointer_abi::Tlv<'a>, VMError> {
    let address = if direct {
        resolver(vm, address)
    } else {
        address
    };
    let tlv = vm.validate_tlv(address)?;
    if tlv.type_id != expected
        || !pointer_abi::is_type_allowed_for_policy(vm.syscall_policy(), tlv.type_id)
    {
        return Err(VMError::NoritoInvalid);
    }
    Ok(tlv)
}

fn decode_canonical<T>(payload: &[u8]) -> Result<T, VMError>
where
    T: for<'de> NoritoDeserialize<'de> + NoritoSerialize,
{
    let value = decode_from_bytes(payload).map_err(|_| VMError::DecodeError)?;
    if to_bytes(&value).map_err(|_| VMError::DecodeError)? != payload {
        return Err(VMError::DecodeError);
    }
    Ok(value)
}

fn allocate_tlv(vm: &mut IVM, pointer_type: PointerType, payload: &[u8]) -> Result<u64, VMError> {
    let payload_len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
    let mut envelope = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    envelope.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    envelope.push(1);
    envelope.extend_from_slice(&payload_len.to_be_bytes());
    envelope.extend_from_slice(payload);
    envelope.extend_from_slice(Hash::new(payload).as_ref());
    vm.alloc_host_tlv(&envelope)
}

fn decode_construction_schema(
    vm: &IVM,
    address: u64,
    resolver: AddressResolver,
) -> Result<(JsonConstructionSchemaV1, usize), VMError> {
    let tlv = load_tlv(vm, address, PointerType::NoritoBytes, resolver)?;
    if tlv.payload.len() > MAX_JSON_CONSTRUCTION_SCHEMA_BYTES_V1 {
        return Err(VMError::DecodeError);
    }
    let schema: JsonConstructionSchemaV1 = decode_canonical(tlv.payload)?;
    if !schema.validate() {
        return Err(VMError::DecodeError);
    }
    Ok((schema, tlv.payload.len()))
}

fn read_word_table(vm: &IVM, address: u64, count: usize) -> Result<Vec<u64>, VMError> {
    if count > MAX_STATE_VALUE_WORDS || !address.is_multiple_of(8) {
        return Err(VMError::DecodeError);
    }
    let byte_len = count.checked_mul(8).ok_or(VMError::DecodeError)?;
    if byte_len == 0 {
        return Ok(Vec::new());
    }
    vm.ensure_public_memory(
        address,
        u64::try_from(byte_len).map_err(|_| VMError::DecodeError)?,
    )?;
    let bytes = vm.memory.load_region(
        address,
        u64::try_from(byte_len).map_err(|_| VMError::DecodeError)?,
    )?;
    Ok(bytes
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("eight-byte word")))
        .collect())
}

fn state_node_word_count(
    nodes: &[StateValueNodeV1],
    node_index: &mut usize,
) -> Result<usize, VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Option => {
            state_node_word_count(nodes, node_index)?;
            Ok(1)
        }
        StateValueNodeV1::List { .. } | StateValueNodeV1::Leaf(_) => Ok(1),
        StateValueNodeV1::Struct { .. }
        | StateValueNodeV1::Tuple { .. }
        | StateValueNodeV1::Result => Err(VMError::DecodeError),
    }
}

fn pointer_leaf<'a>(
    vm: &'a IVM,
    word: u64,
    pointer_type: PointerType,
    resolver: AddressResolver,
    stats: &mut BuildStats,
) -> Result<&'a [u8], VMError> {
    let tlv = load_tlv(vm, word, pointer_type, resolver)?;
    stats.source_bytes = stats.source_bytes.saturating_add(tlv.payload.len());
    Ok(tlv.payload)
}

fn convert_leaf(
    vm: &IVM,
    kind: StateValueKindV1,
    word: u64,
    resolver: AddressResolver,
    stats: &mut BuildStats,
) -> Result<njson::Value, VMError> {
    if crate::dev_env::decode_trace_enabled() {
        eprintln!("[json] convert leaf: kind={kind:?} word=0x{word:x}");
    }
    Ok(match kind {
        StateValueKindV1::Int => njson::Value::from(word as i64),
        StateValueKindV1::Bool => match word {
            0 => njson::Value::Bool(false),
            1 => njson::Value::Bool(true),
            _ => return Err(VMError::DecodeError),
        },
        StateValueKindV1::U128 => {
            let payload = pointer_leaf(vm, word, PointerType::NoritoBytes, resolver, stats)?;
            let numeric: Numeric = decode_canonical(payload)?;
            if numeric.scale() != 0 {
                return Err(VMError::DecodeError);
            }
            let value = numeric.try_mantissa_u128().ok_or(VMError::DecodeError)?;
            match u64::try_from(value) {
                Ok(value) => njson::Value::from(value),
                Err(_) => njson::Value::from(value.to_string()),
            }
        }
        StateValueKindV1::Amount => {
            let payload = pointer_leaf(vm, word, PointerType::Amount, resolver, stats)?;
            let amount: Numeric = decode_canonical(payload)?;
            amount.validate_amount().map_err(|_| VMError::DecodeError)?;
            njson::Value::from(amount.to_string())
        }
        StateValueKindV1::String => {
            let payload = pointer_leaf(vm, word, PointerType::Blob, resolver, stats)?;
            njson::Value::from(
                core::str::from_utf8(payload)
                    .map_err(|_| VMError::DecodeError)?
                    .to_owned(),
            )
        }
        StateValueKindV1::Json => {
            let payload = pointer_leaf(vm, word, PointerType::Json, resolver, stats)?;
            let json: Json = decode_canonical(payload)?;
            json.try_into_any_norito()
                .map_err(|_| VMError::DecodeError)?
        }
        StateValueKindV1::Bytes => {
            let payload = pointer_leaf(vm, word, PointerType::Blob, resolver, stats)?;
            njson::Value::from(format!("0x{}", hex::encode(payload)))
        }
        StateValueKindV1::AccountId => {
            let payload = pointer_leaf(vm, word, PointerType::AccountId, resolver, stats)?;
            njson::Value::from(decode_canonical::<AccountId>(payload)?.to_string())
        }
        StateValueKindV1::AssetDefinitionId => {
            let payload = pointer_leaf(vm, word, PointerType::AssetDefinitionId, resolver, stats)?;
            njson::Value::from(decode_canonical::<AssetDefinitionId>(payload)?.to_string())
        }
        StateValueKindV1::AssetId => {
            let payload = pointer_leaf(vm, word, PointerType::AssetId, resolver, stats)?;
            njson::Value::from(decode_canonical::<AssetId>(payload)?.to_string())
        }
        StateValueKindV1::DomainId => {
            let payload = pointer_leaf(vm, word, PointerType::DomainId, resolver, stats)?;
            njson::Value::from(decode_canonical::<DomainId>(payload)?.to_string())
        }
        StateValueKindV1::NftId => {
            let payload = pointer_leaf(vm, word, PointerType::NftId, resolver, stats)?;
            njson::Value::from(decode_canonical::<NftId>(payload)?.to_string())
        }
        StateValueKindV1::Name => {
            let payload = pointer_leaf(vm, word, PointerType::Name, resolver, stats)?;
            njson::Value::from(decode_canonical::<Name>(payload)?.to_string())
        }
        StateValueKindV1::DataSpaceId => {
            let payload = pointer_leaf(vm, word, PointerType::DataSpaceId, resolver, stats)?;
            njson::Value::from(decode_canonical::<DataSpaceId>(payload)?.to_string())
        }
        StateValueKindV1::AxtDescriptor
        | StateValueKindV1::AssetHandle
        | StateValueKindV1::ProofBlob
        | StateValueKindV1::SoracloudRequest
        | StateValueKindV1::SoracloudResponse => return Err(VMError::DecodeError),
    })
}

fn convert_state_node(
    vm: &IVM,
    nodes: &[StateValueNodeV1],
    node_index: &mut usize,
    words: &[u64],
    word_index: &mut usize,
    resolver: AddressResolver,
    stats: &mut BuildStats,
) -> Result<njson::Value, VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Option => {
            let handle = *words.get(*word_index).ok_or(VMError::DecodeError)?;
            if crate::dev_env::decode_trace_enabled() {
                eprintln!("[json] convert Option handle=0x{handle:x}");
            }
            *word_index = word_index.saturating_add(1);
            let child_start = *node_index;
            let mut child_end = child_start;
            let child_words = state_node_word_count(nodes, &mut child_end)?;
            let layout = crate::sum::SumLayoutV1::option(
                u64::try_from(child_words).map_err(|_| VMError::DecodeError)?,
            )
            .map_err(|_| VMError::DecodeError)?;
            let (some, payload) = crate::sum::read_words(vm, handle, layout)?;
            if !some {
                *node_index = child_end;
                return Ok(njson::Value::Null);
            }
            let mut payload_index = 0;
            let value = convert_state_node(
                vm,
                nodes,
                node_index,
                &payload,
                &mut payload_index,
                resolver,
                stats,
            )?;
            if *node_index != child_end || payload_index != payload.len() {
                return Err(VMError::DecodeError);
            }
            Ok(value)
        }
        StateValueNodeV1::List { element, capacity } => {
            let handle = *words.get(*word_index).ok_or(VMError::DecodeError)?;
            if crate::dev_env::decode_trace_enabled() {
                eprintln!(
                    "[json] convert List handle=0x{handle:x} capacity={capacity} element_words={:?}",
                    element.word_count()
                );
            }
            *word_index = word_index.saturating_add(1);
            let element_words = element.word_count().ok_or(VMError::DecodeError)?;
            let layout = crate::list::ListLayoutV1::try_new(
                u64::from(*capacity),
                u64::try_from(element_words).map_err(|_| VMError::DecodeError)?,
            )
            .map_err(|_| VMError::DecodeError)?;
            let items = crate::list::read_words(vm, handle, layout)?;
            stats.collection_elements = stats.collection_elements.saturating_add(items.len());
            let values = items
                .into_iter()
                .map(|item| convert_state_schema(vm, element, &item, resolver, stats))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(njson::Value::Array(values))
        }
        StateValueNodeV1::Leaf(kind) => {
            let word = *words.get(*word_index).ok_or(VMError::DecodeError)?;
            *word_index = word_index.saturating_add(1);
            convert_leaf(vm, *kind, word, resolver, stats)
        }
        StateValueNodeV1::Struct { .. }
        | StateValueNodeV1::Tuple { .. }
        | StateValueNodeV1::Result => Err(VMError::DecodeError),
    }
}

fn convert_state_schema(
    vm: &IVM,
    schema: &StateValueSchemaV1,
    words: &[u64],
    resolver: AddressResolver,
    stats: &mut BuildStats,
) -> Result<njson::Value, VMError> {
    let mut node_index = 0;
    let mut word_index = 0;
    let value = convert_state_node(
        vm,
        &schema.nodes,
        &mut node_index,
        words,
        &mut word_index,
        resolver,
        stats,
    )?;
    if node_index != schema.nodes.len() || word_index != words.len() {
        return Err(VMError::DecodeError);
    }
    Ok(value)
}

fn convert_construction_node(
    vm: &IVM,
    nodes: &[JsonConstructionNodeV1],
    node_index: &mut usize,
    words: &[u64],
    word_index: &mut usize,
    resolver: AddressResolver,
    stats: &mut BuildStats,
) -> Result<njson::Value, VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        JsonConstructionNodeV1::Object { keys } => {
            stats.collection_elements = stats.collection_elements.saturating_add(keys.len());
            let mut object = njson::Map::new();
            for key in keys {
                let value = convert_construction_node(
                    vm, nodes, node_index, words, word_index, resolver, stats,
                )?;
                object.insert(key.clone(), value);
            }
            Ok(njson::Value::Object(object))
        }
        JsonConstructionNodeV1::Array { arity } => {
            stats.collection_elements = stats
                .collection_elements
                .saturating_add(usize::from(*arity));
            let mut values = Vec::with_capacity(usize::from(*arity));
            for _ in 0..*arity {
                values.push(convert_construction_node(
                    vm, nodes, node_index, words, word_index, resolver, stats,
                )?);
            }
            Ok(njson::Value::Array(values))
        }
        JsonConstructionNodeV1::Value { schema } => {
            let count = schema.word_count().ok_or(VMError::DecodeError)?;
            let end = word_index.checked_add(count).ok_or(VMError::DecodeError)?;
            let value_words = words.get(*word_index..end).ok_or(VMError::DecodeError)?;
            *word_index = end;
            convert_state_schema(vm, schema, value_words, resolver, stats)
        }
    }
}

/// Deterministic gas charged by [`build_json`].
#[must_use]
pub fn build_json_gas(
    schema_bytes: usize,
    source_bytes: usize,
    words: usize,
    collection_elements: usize,
    output_bytes: usize,
) -> u64 {
    [
        schema_bytes,
        source_bytes,
        words,
        collection_elements,
        output_bytes,
    ]
    .into_iter()
    .fold(JSON_BUILD_GAS_BASE, |gas, value| {
        gas.saturating_add(u64::try_from(value).unwrap_or(u64::MAX))
    })
}

/// Deterministic gas for one typed JSON getter.
#[must_use]
pub fn typed_getter_gas(input_bytes: usize, output_bytes: usize) -> u64 {
    JSON_TYPED_GETTER_GAS_BASE
        .saturating_add(u64::try_from(input_bytes).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(output_bytes).unwrap_or(u64::MAX))
}

/// Execute `JSON_BUILD` using `r10=schema`, `r11=word table`, `r12=word count`.
///
/// # Errors
///
/// Returns a deterministic VM error for a malformed schema, word table, handle,
/// pointer payload, or unsupported source type.
pub fn build_json(vm: &mut IVM, resolver: AddressResolver) -> Result<u64, VMError> {
    let (schema, schema_bytes) = decode_construction_schema(vm, vm.register(10), resolver)?;
    if crate::dev_env::decode_trace_enabled() {
        eprintln!(
            "[json] decoded construction schema: nodes={} bytes={schema_bytes}",
            schema.nodes.len()
        );
    }
    let expected_words = schema.word_count().ok_or(VMError::DecodeError)?;
    let supplied_words = usize::try_from(vm.register(12)).map_err(|_| VMError::DecodeError)?;
    if supplied_words != expected_words {
        return Err(VMError::DecodeError);
    }
    let words = read_word_table(vm, vm.register(11), supplied_words)?;
    if crate::dev_env::decode_trace_enabled() {
        eprintln!(
            "[json] read construction word table: expected={expected_words} supplied={supplied_words} words={words:x?}"
        );
    }
    let mut stats = BuildStats::default();
    let mut node_index = 0;
    let mut word_index = 0;
    let value = convert_construction_node(
        vm,
        &schema.nodes,
        &mut node_index,
        &words,
        &mut word_index,
        resolver,
        &mut stats,
    )?;
    if crate::dev_env::decode_trace_enabled() {
        eprintln!(
            "[json] converted construction tree: nodes={node_index}/{} words={word_index}/{}",
            schema.nodes.len(),
            words.len()
        );
    }
    if node_index != schema.nodes.len() || word_index != words.len() {
        return Err(VMError::DecodeError);
    }
    let json = Json::from(value);
    let payload = to_bytes(&json).map_err(|_| VMError::NoritoInvalid)?;
    if crate::dev_env::decode_trace_enabled() {
        eprintln!("[json] encoded JSON payload: bytes={}", payload.len());
    }
    let gas = build_json_gas(
        schema_bytes,
        stats.source_bytes,
        supplied_words,
        stats.collection_elements,
        payload.len(),
    );
    preflight_reserved_syscall_gas(vm, gas)?;
    let pointer = allocate_tlv(vm, PointerType::Json, &payload)?;
    if crate::dev_env::decode_trace_enabled() {
        eprintln!("[json] allocated JSON result: pointer=0x{pointer:x}");
    }
    vm.set_register(10, pointer);
    Ok(gas)
}

fn canonical_account(raw: &str) -> Option<AccountId> {
    let value = AccountId::parse_encoded(raw)
        .ok()
        .map(ParsedAccountId::into_account_id)?;
    (value.to_string() == raw).then_some(value)
}

fn canonical_asset_definition(raw: &str) -> Option<AssetDefinitionId> {
    let value = AssetDefinitionId::parse_address_literal(raw).ok()?;
    (value.to_string() == raw).then_some(value)
}

fn canonical_from_str<T>(raw: &str) -> Option<T>
where
    T: FromStr + ToString,
{
    let value = raw.parse::<T>().ok()?;
    (value.to_string() == raw).then_some(value)
}

fn canonical_hex_bytes(raw: &str) -> Option<Vec<u8>> {
    let hex = raw.strip_prefix("0x")?;
    if hex.len() % 2 != 0
        || !hex
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return None;
    }
    hex::decode(hex).ok()
}

fn amount_field(field: &njson::Value) -> Option<Numeric> {
    let value = match field {
        njson::Value::String(raw) => raw.parse::<Numeric>().ok()?,
        njson::Value::Number(njson::native::Number::I64(value)) => Numeric::from(*value),
        njson::Value::Number(njson::native::Number::U64(value)) => Numeric::from(*value),
        _ => return None,
    }
    .canonicalize_amount()
    .ok()?;
    value.validate_amount().ok()?;
    Some(value)
}

enum GetterValue {
    Word(u64),
    Pointer(PointerType, Vec<u8>),
}

fn getter_value(number: u32, field: &njson::Value) -> Option<GetterValue> {
    Some(match number {
        syscalls::SYSCALL_JSON_GET_I64 => {
            let value = match field {
                njson::Value::Number(njson::native::Number::I64(value)) => *value,
                njson::Value::Number(njson::native::Number::U64(value)) => {
                    i64::try_from(*value).ok()?
                }
                _ => return None,
            };
            GetterValue::Word(value as u64)
        }
        syscalls::SYSCALL_JSON_GET_JSON => {
            let json = Json::from_norito_value_ref(field).ok()?;
            GetterValue::Pointer(PointerType::Json, to_bytes(&json).ok()?)
        }
        syscalls::SYSCALL_JSON_GET_NAME => {
            let raw = field.as_str()?;
            let value = Name::from_str(raw).ok()?;
            if value.as_ref() != raw {
                return None;
            }
            GetterValue::Pointer(PointerType::Name, to_bytes(&value).ok()?)
        }
        syscalls::SYSCALL_JSON_GET_ACCOUNT_ID => GetterValue::Pointer(
            PointerType::AccountId,
            to_bytes(&canonical_account(field.as_str()?)?).ok()?,
        ),
        syscalls::SYSCALL_JSON_GET_NFT_ID => GetterValue::Pointer(
            PointerType::NftId,
            to_bytes(&canonical_from_str::<NftId>(field.as_str()?)?).ok()?,
        ),
        syscalls::SYSCALL_JSON_GET_BLOB_HEX => {
            GetterValue::Pointer(PointerType::Blob, canonical_hex_bytes(field.as_str()?)?)
        }
        syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID => GetterValue::Pointer(
            PointerType::AssetDefinitionId,
            to_bytes(&canonical_asset_definition(field.as_str()?)?).ok()?,
        ),
        syscalls::SYSCALL_JSON_GET_AMOUNT => {
            GetterValue::Pointer(PointerType::Amount, to_bytes(&amount_field(field)?).ok()?)
        }
        _ => return None,
    })
}

/// Execute one typed JSON getter and materialize `Option<T>` as an active-only
/// compiler-owned sum handle in `r10`.
///
/// Missing fields, non-object roots, and conversion/type mismatches produce
/// `Option::none`. Malformed pointer envelopes or noncanonical root/key payloads
/// remain deterministic VM errors.
pub fn typed_getter(
    vm: &mut IVM,
    number: u32,
    resolver: AddressResolver,
) -> Result<JsonGetterCost, VMError> {
    let canonical = syscalls::canonical_helper_syscall(number);
    if !syscalls::is_json_getter_syscall(canonical) {
        return Err(VMError::UnknownSyscall(number));
    }
    let direct = number != canonical;
    let json_tlv = load_getter_tlv(vm, vm.register(10), PointerType::Json, direct, resolver)?;
    let key_tlv = load_getter_tlv(vm, vm.register(11), PointerType::Name, direct, resolver)?;
    let json: Json = decode_canonical(json_tlv.payload)?;
    let key: Name = decode_canonical(key_tlv.payload)?;
    let value: njson::Value = json
        .try_into_any_norito()
        .map_err(|_| VMError::DecodeError)?;
    let converted = value
        .as_object()
        .and_then(|object| object.get(key.as_ref()))
        .and_then(|field| getter_value(canonical, field));
    let input_bytes = json_tlv.payload.len().saturating_add(key_tlv.payload.len());
    let layout = crate::sum::SumLayoutV1::option(1).map_err(|_| VMError::DecodeError)?;
    let (handle, payload_bytes) = match converted {
        Some(GetterValue::Word(word)) => (
            crate::sum::allocate_words(vm, layout, 1, &[word])?,
            core::mem::size_of::<u64>(),
        ),
        Some(GetterValue::Pointer(pointer_type, payload)) => {
            let pointer = allocate_tlv(vm, pointer_type, &payload)?;
            (
                crate::sum::allocate_words(vm, layout, 1, &[pointer])?,
                payload.len(),
            )
        }
        None => (crate::sum::allocate_words(vm, layout, 0, &[])?, 0),
    };
    vm.set_register(10, handle);
    Ok(JsonGetterCost {
        input_bytes,
        output_bytes: payload_bytes.saturating_add(16),
    })
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use ivm_abi::{
        json::{JsonConstructionNodeV1, JsonConstructionSchemaV1},
        state_value::{StateValueKindV1, StateValueNodeV1, StateValueSchemaV1},
    };

    use super::*;
    use crate::{core_host::CoreHost, memory::Memory};

    fn tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
        bytes.extend_from_slice(&(pointer_type as u16).to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("test payload length")
                .to_be_bytes(),
        );
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(Hash::new(payload).as_ref());
        bytes
    }

    fn leaf(kind: StateValueKindV1) -> StateValueSchemaV1 {
        StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(kind)],
        }
    }

    #[test]
    fn build_json_orders_keys_and_converts_amount_and_bytes() {
        let account = AccountId::new(
            KeyPair::random_with_algorithm(Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let account_payload = to_bytes(&account).expect("encode account");
        let amount = Numeric::new(125_u32, 2);
        let amount_payload = to_bytes(&amount).expect("encode amount");
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Object {
                    keys: vec!["z_owner".into(), "a_amount".into(), "bytes".into()],
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::AccountId),
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Amount),
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Bytes),
                },
            ],
        };
        let schema_payload = to_bytes(&schema).expect("encode construction schema");
        let mut vm = IVM::new(u64::MAX);
        let schema_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::NoritoBytes, &schema_payload))
            .expect("schema TLV");
        let account_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::AccountId, &account_payload))
            .expect("account TLV");
        let amount_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Amount, &amount_payload))
            .expect("amount TLV");
        let bytes_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Blob, &[0xab, 0x01]))
            .expect("bytes TLV");
        let table = vm.alloc_heap(24).expect("word table");
        for (index, word) in [account_ptr, amount_ptr, bytes_ptr].into_iter().enumerate() {
            vm.store_u64(table + u64::try_from(index).unwrap() * 8, word)
                .expect("table word");
        }
        vm.set_register(10, schema_ptr);
        vm.set_register(11, table);
        vm.set_register(12, 3);
        let gas = build_json(&mut vm, CoreHost::resolve_code_tlv_addr).expect("build JSON");
        let output = vm.validate_tlv(vm.register(10)).expect("JSON output");
        assert_eq!(
            gas,
            build_json_gas(
                schema_payload.len(),
                account_payload.len() + amount_payload.len() + 2,
                3,
                3,
                output.payload.len(),
            )
        );
        let json: Json = decode_from_bytes(output.payload).expect("decode JSON");
        let text = njson::to_string(&json).expect("render JSON");
        assert!(text.find("a_amount").unwrap() < text.find("z_owner").unwrap());
        assert!(text.contains(r#""a_amount":"1.25""#));
        assert!(text.contains(r#""bytes":"0xab01""#));
        assert!(text.contains(&account.to_string()));
    }

    #[test]
    fn build_json_recurses_through_list_and_active_only_option() {
        let amount = Numeric::new(125_u32, 2);
        let amount_payload = to_bytes(&amount).expect("encode amount");
        let element_schema = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Amount),
            ],
        };
        let list_schema = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(element_schema),
                capacity: 2,
            }],
        };
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![JsonConstructionNodeV1::Value {
                schema: list_schema,
            }],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&schema).expect("schema"),
            ))
            .expect("schema TLV");
        let amount_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Amount, &amount_payload))
            .expect("amount TLV");
        let option_layout = crate::sum::SumLayoutV1::option(1).expect("option layout");
        let some = crate::sum::allocate_words(&mut vm, option_layout, 1, &[amount_ptr])
            .expect("some amount");
        let none = crate::sum::allocate_words(&mut vm, option_layout, 0, &[]).expect("none amount");
        let list_layout = crate::list::ListLayoutV1::try_new(2, 1).expect("list layout");
        let list = crate::list::allocate_words(&mut vm, list_layout, &[vec![some], vec![none]])
            .expect("option list");
        let table = vm.alloc_heap(8).expect("word table");
        vm.store_u64(table, list).expect("list word");
        vm.set_register(10, schema_ptr);
        vm.set_register(11, table);
        vm.set_register(12, 1);
        build_json(&mut vm, CoreHost::resolve_code_tlv_addr).expect("build option list JSON");
        let output = vm.validate_tlv(vm.register(10)).expect("JSON output");
        let json: Json = decode_from_bytes(output.payload).expect("decode JSON");
        let value: njson::Value = json.try_into_any_norito().expect("JSON value");
        assert_eq!(
            value,
            njson::Value::Array(vec![njson::Value::from("1.25"), njson::Value::Null])
        );
    }

    #[test]
    fn build_json_preserves_max_u128_and_scale_28_amount_without_floats() {
        let maximum = Numeric::new(u128::MAX, 0);
        let precise = Numeric::new(1_u32, 28);
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Array { arity: 2 },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::U128),
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Amount),
                },
            ],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&schema).expect("schema"),
            ))
            .expect("schema TLV");
        let maximum_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&maximum).expect("maximum u128"),
            ))
            .expect("u128 TLV");
        let precise_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::Amount,
                &to_bytes(&precise).expect("scale-28 amount"),
            ))
            .expect("Amount TLV");
        let table = vm.alloc_heap(16).expect("word table");
        vm.store_u64(table, maximum_ptr).expect("u128 word");
        vm.store_u64(table + 8, precise_ptr).expect("Amount word");
        vm.set_register(10, schema_ptr);
        vm.set_register(11, table);
        vm.set_register(12, 2);
        build_json(&mut vm, CoreHost::resolve_code_tlv_addr).expect("build exact JSON");
        let output = vm.validate_tlv(vm.register(10)).expect("JSON output");
        let json: Json = decode_from_bytes(output.payload).expect("decode JSON");
        let value: njson::Value = json.try_into_any_norito().expect("JSON value");
        assert_eq!(
            value,
            njson::Value::Array(vec![
                njson::Value::from(u128::MAX.to_string()),
                njson::Value::from("0.0000000000000000000000000001"),
            ])
        );
    }

    #[test]
    fn typed_getter_returns_active_only_some_and_none_handles() {
        let json = Json::from(norito::json!({"count": 7, "wrong": true}));
        let json_payload = to_bytes(&json).expect("encode JSON");
        let mut vm = IVM::new(u64::MAX);
        let json_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Json, &json_payload))
            .expect("JSON TLV");
        let key: Name = "count".parse().expect("key");
        let key_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Name, &to_bytes(&key).unwrap()))
            .expect("key TLV");
        vm.set_register(10, json_ptr);
        vm.set_register(11, key_ptr);
        typed_getter(
            &mut vm,
            syscalls::SYSCALL_JSON_GET_I64,
            CoreHost::resolve_code_tlv_addr,
        )
        .expect("get count");
        assert_eq!(
            crate::sum::read_words(
                &vm,
                vm.register(10),
                crate::sum::SumLayoutV1::option(1).unwrap()
            ),
            Ok((true, vec![7]))
        );

        let missing: Name = "missing".parse().expect("missing key");
        let missing_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Name, &to_bytes(&missing).unwrap()))
            .expect("missing key TLV");
        vm.set_register(10, json_ptr);
        vm.set_register(11, missing_ptr);
        typed_getter(
            &mut vm,
            syscalls::SYSCALL_JSON_GET_I64,
            CoreHost::resolve_code_tlv_addr,
        )
        .expect("missing is none");
        assert_eq!(
            crate::sum::read_words(
                &vm,
                vm.register(10),
                crate::sum::SumLayoutV1::option(1).unwrap()
            ),
            Ok((false, vec![]))
        );

        let wrong: Name = "wrong".parse().expect("wrong-type key");
        let wrong_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Name, &to_bytes(&wrong).unwrap()))
            .expect("wrong-type key TLV");
        vm.set_register(10, json_ptr);
        vm.set_register(11, wrong_ptr);
        typed_getter(
            &mut vm,
            syscalls::SYSCALL_JSON_GET_I64,
            CoreHost::resolve_code_tlv_addr,
        )
        .expect("wrong type is none");
        assert_eq!(
            crate::sum::read_words(
                &vm,
                vm.register(10),
                crate::sum::SumLayoutV1::option(1).unwrap()
            ),
            Ok((false, vec![]))
        );

        let array = Json::from(norito::json!([7]));
        let array_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::Json,
                &to_bytes(&array).expect("encode array JSON"),
            ))
            .expect("array JSON TLV");
        vm.set_register(10, array_ptr);
        vm.set_register(11, key_ptr);
        typed_getter(
            &mut vm,
            syscalls::SYSCALL_JSON_GET_I64,
            CoreHost::resolve_code_tlv_addr,
        )
        .expect("non-object root is none");
        assert_eq!(
            crate::sum::read_words(
                &vm,
                vm.register(10),
                crate::sum::SumLayoutV1::option(1).unwrap()
            ),
            Ok((false, vec![]))
        );
    }

    #[test]
    fn typed_amount_getter_canonicalizes_valid_values_and_rejects_invalid_ones() {
        let oversized = "9".repeat(200);
        let json = Json::from(norito::json!({
            "decimal": "1.2500",
            "integer": 7,
            "negative": "-1",
            "oversized": oversized,
            "wrong": true,
        }));
        let json_payload = to_bytes(&json).expect("encode JSON");
        let mut vm = IVM::new(u64::MAX);
        let json_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Json, &json_payload))
            .expect("JSON TLV");
        let option_layout = crate::sum::SumLayoutV1::option(1).expect("Option<Amount> layout");

        for (key, expected) in [
            ("decimal", Some(Numeric::new(125_u32, 2))),
            ("integer", Some(Numeric::new(7_u32, 0))),
            ("negative", None),
            ("oversized", None),
            ("wrong", None),
            ("missing", None),
        ] {
            let key: Name = key.parse().expect("valid key");
            let key_ptr = vm
                .alloc_input_tlv(&tlv(
                    PointerType::Name,
                    &to_bytes(&key).expect("encode key"),
                ))
                .expect("key TLV");
            vm.set_register(10, json_ptr);
            vm.set_register(11, key_ptr);
            typed_getter(
                &mut vm,
                syscalls::SYSCALL_JSON_GET_AMOUNT,
                CoreHost::resolve_code_tlv_addr,
            )
            .expect("typed Amount getter");
            let (some, payload) = crate::sum::read_words(&vm, vm.register(10), option_layout)
                .expect("read Option<Amount>");
            match expected {
                Some(expected) => {
                    assert!(some, "{key} must produce Option::some");
                    let amount = vm.validate_tlv(payload[0]).expect("Amount TLV");
                    assert_eq!(amount.type_id, PointerType::Amount);
                    let amount: Numeric =
                        decode_from_bytes(amount.payload).expect("decode canonical Amount");
                    assert_eq!(amount, expected);
                    amount.validate_amount().expect("canonical Amount payload");
                }
                None => {
                    assert!(!some, "{key} must produce Option::none");
                    assert!(payload.is_empty(), "none has no placeholder payload");
                }
            }
        }
    }

    #[test]
    fn build_json_rejects_duplicate_key_schema_before_reading_values() {
        let duplicate = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Object {
                    keys: vec!["same".into(), "same".into()],
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Int),
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Int),
                },
            ],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&duplicate).expect("encode duplicate schema"),
            ))
            .expect("schema TLV");
        vm.set_register(10, schema_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, 2);

        assert_eq!(
            build_json(&mut vm, CoreHost::resolve_code_tlv_addr),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn build_json_rejects_noncanonical_amounts_and_hidden_option_payloads() {
        let amount_schema = JsonConstructionSchemaV1 {
            nodes: vec![JsonConstructionNodeV1::Value {
                schema: leaf(StateValueKindV1::Amount),
            }],
        };
        let mut vm = IVM::new(u64::MAX);
        let amount_schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&amount_schema).expect("Amount schema"),
            ))
            .expect("Amount schema TLV");
        let noncanonical = vm
            .alloc_input_tlv(&tlv(
                PointerType::Amount,
                &to_bytes(&Numeric::new(10_u32, 1)).expect("noncanonical Amount payload"),
            ))
            .expect("noncanonical Amount TLV");
        let amount_table = vm.alloc_heap(8).expect("Amount word table");
        vm.store_u64(amount_table, noncanonical)
            .expect("store Amount pointer");
        vm.set_register(10, amount_schema_ptr);
        vm.set_register(11, amount_table);
        vm.set_register(12, 1);
        assert_eq!(
            build_json(&mut vm, CoreHost::resolve_code_tlv_addr),
            Err(VMError::DecodeError),
            "Amount inputs must already use their unique canonical payload",
        );

        let option_schema = JsonConstructionSchemaV1 {
            nodes: vec![JsonConstructionNodeV1::Value {
                schema: StateValueSchemaV1 {
                    nodes: vec![
                        StateValueNodeV1::Option,
                        StateValueNodeV1::Leaf(StateValueKindV1::Amount),
                    ],
                },
            }],
        };
        let option_schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&option_schema).expect("Option schema"),
            ))
            .expect("Option schema TLV");
        let option_layout = crate::sum::SumLayoutV1::option(1).expect("Option<Amount> layout");
        let none = crate::sum::allocate_words(&mut vm, option_layout, 0, &[])
            .expect("canonical Option::none");
        vm.store_u64(none + 8, noncanonical)
            .expect("forge inactive placeholder payload");
        let option_table = vm.alloc_heap(8).expect("Option word table");
        vm.store_u64(option_table, none)
            .expect("store Option handle");
        vm.set_register(10, option_schema_ptr);
        vm.set_register(11, option_table);
        vm.set_register(12, 1);
        assert_eq!(
            build_json(&mut vm, CoreHost::resolve_code_tlv_addr),
            Err(VMError::DecodeError),
            "Option::none cannot smuggle an inactive placeholder into JSON",
        );
    }

    #[test]
    fn build_json_rejects_a_word_count_that_disagrees_with_the_schema() {
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![JsonConstructionNodeV1::Value {
                schema: leaf(StateValueKindV1::Int),
            }],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&schema).expect("encode schema"),
            ))
            .expect("schema TLV");
        vm.set_register(10, schema_ptr);
        vm.set_register(11, 0);
        vm.set_register(12, 0);

        assert_eq!(
            build_json(&mut vm, CoreHost::resolve_code_tlv_addr),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn build_json_spills_large_canonical_output_to_owned_heap() {
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Array { arity: 1 },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::String),
                },
            ],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&schema).expect("encode schema"),
            ))
            .expect("schema TLV");
        let source = "x".repeat(70 * 1024);
        let source_ptr = vm
            .alloc_host_tlv(&tlv(PointerType::Blob, source.as_bytes()))
            .expect("large string TLV");
        let table = vm.alloc_heap(8).expect("word table");
        vm.store_u64(table, source_ptr).expect("string word");
        vm.set_register(10, schema_ptr);
        vm.set_register(11, table);
        vm.set_register(12, 1);

        build_json(&mut vm, CoreHost::resolve_code_tlv_addr).expect("build large JSON");
        let output_ptr = vm.register(10);
        assert!(output_ptr >= Memory::HEAP_START);
        let output = vm.validate_tlv(output_ptr).expect("large JSON output");
        assert_eq!(output.type_id, PointerType::Json);
        let json: Json = decode_from_bytes(output.payload).expect("decode large JSON");
        let value: njson::Value = json.try_into_any_norito().expect("large JSON value");
        assert_eq!(
            value.as_array().and_then(|items| items[0].as_str()),
            Some(source.as_str())
        );
    }

    #[test]
    fn typed_getter_rejects_malformed_json_and_key_payloads() {
        let mut vm = IVM::new(u64::MAX);
        let malformed_json = vm
            .alloc_input_tlv(&tlv(PointerType::Json, br#"{"count":7}"#))
            .expect("malformed typed JSON TLV");
        let key: Name = "count".parse().expect("key");
        let key_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Name, &to_bytes(&key).unwrap()))
            .expect("key TLV");
        vm.set_register(10, malformed_json);
        vm.set_register(11, key_ptr);
        assert_eq!(
            typed_getter(
                &mut vm,
                syscalls::SYSCALL_JSON_GET_I64,
                CoreHost::resolve_code_tlv_addr,
            ),
            Err(VMError::DecodeError)
        );

        let json = Json::from(norito::json!({"count": 7}));
        let json_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::Json,
                &to_bytes(&json).expect("encode JSON"),
            ))
            .expect("JSON TLV");
        let malformed_key = vm
            .alloc_input_tlv(&tlv(PointerType::Name, b"count"))
            .expect("malformed typed key TLV");
        vm.set_register(10, json_ptr);
        vm.set_register(11, malformed_key);
        assert_eq!(
            typed_getter(
                &mut vm,
                syscalls::SYSCALL_JSON_GET_I64,
                CoreHost::resolve_code_tlv_addr,
            ),
            Err(VMError::DecodeError)
        );
    }
}
