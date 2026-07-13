//! Native, schema-bound Kotodama JSON construction and typed getters.
//!
//! Integers use canonical JSON number tokens across the complete `i64`/`u64`
//! domain. Exact decimals and quantities use canonical strings so they never
//! pass through floating-point conversion.

use core::str::FromStr;

use iroha_crypto::Hash;
use iroha_data_model::{
    account::{AccountId, ParsedAccountId},
    prelude::{AssetDefinitionId, AssetId, DataSpaceId, DomainId, Name, NftId},
};
use iroha_primitives::{
    bigint::BigInt,
    json::Json,
    numeric::{Numeric, Quantity},
    numeric_abi::{DecimalValueV1, IntValueV1, QuantityValueV1},
};
use ivm_abi::{
    json::{
        JsonConstructionNodeV1, JsonConstructionSchemaV1, MAX_JSON_CONSTRUCTION_SCHEMA_BYTES_V1,
    },
    state_value::{MAX_STATE_VALUE_WORDS, StateValueKindV1, StateValueNodeV1, StateValueSchemaV1},
};
use norito::{
    NoritoDeserialize, core::NoritoSerialize, decode_from_bytes, json as njson,
    json::native::Number as JsonNumber, to_bytes,
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
        StateValueKindV1::Int => {
            let payload = pointer_leaf(vm, word, PointerType::Int, resolver, stats)?;
            let value = IntValueV1::decode_frame(payload)
                .map_err(|_| VMError::DecodeError)?
                .into_int();
            let spelling = value.to_string();
            if let Ok(value) = spelling.parse::<u64>() {
                njson::Value::from(value)
            } else if let Ok(value) = spelling.parse::<i64>() {
                njson::Value::from(value)
            } else {
                return Err(VMError::DecodeError);
            }
        }
        StateValueKindV1::Bool => match word {
            0 => njson::Value::Bool(false),
            1 => njson::Value::Bool(true),
            _ => return Err(VMError::DecodeError),
        },
        StateValueKindV1::Decimal => {
            let payload = pointer_leaf(vm, word, PointerType::Decimal, resolver, stats)?;
            let value = DecimalValueV1::decode_frame(payload)
                .map_err(|_| VMError::DecodeError)?
                .into_numeric();
            njson::Value::from(value.to_string())
        }
        StateValueKindV1::Quantity => {
            let payload = pointer_leaf(vm, word, PointerType::Quantity, resolver, stats)?;
            let quantity = QuantityValueV1::decode_frame(payload)
                .map_err(|_| VMError::DecodeError)?
                .into_quantity();
            njson::Value::from(quantity.to_string())
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

fn canonical_numeric_string<T>(field: &njson::Value) -> Option<T>
where
    T: FromStr + ToString,
{
    let spelling = field.as_str()?;
    let value = spelling.parse::<T>().ok()?;
    (value.to_string() == spelling).then_some(value)
}

fn canonical_json_integer(field: &njson::Value) -> Option<BigInt> {
    match field {
        njson::Value::Number(JsonNumber::I64(value)) => Some(BigInt::from(*value)),
        njson::Value::Number(JsonNumber::U64(value)) => Some(BigInt::from(*value)),
        njson::Value::Number(JsonNumber::F64(_))
        | njson::Value::Null
        | njson::Value::Bool(_)
        | njson::Value::String(_)
        | njson::Value::Array(_)
        | njson::Value::Object(_) => None,
    }
}

fn getter_value(number: u32, field: &njson::Value) -> Option<(PointerType, Vec<u8>)> {
    Some(match number {
        syscalls::SYSCALL_JSON_GET_JSON => {
            let json = Json::from_norito_value_ref(field).ok()?;
            (PointerType::Json, to_bytes(&json).ok()?)
        }
        syscalls::SYSCALL_JSON_GET_NAME => {
            let raw = field.as_str()?;
            let value = Name::from_str(raw).ok()?;
            if value.as_ref() != raw {
                return None;
            }
            (PointerType::Name, to_bytes(&value).ok()?)
        }
        syscalls::SYSCALL_JSON_GET_ACCOUNT_ID => (
            PointerType::AccountId,
            to_bytes(&canonical_account(field.as_str()?)?).ok()?,
        ),
        syscalls::SYSCALL_JSON_GET_NFT_ID => (
            PointerType::NftId,
            to_bytes(&canonical_from_str::<NftId>(field.as_str()?)?).ok()?,
        ),
        syscalls::SYSCALL_JSON_GET_BLOB_HEX => {
            (PointerType::Blob, canonical_hex_bytes(field.as_str()?)?)
        }
        syscalls::SYSCALL_JSON_GET_ASSET_DEFINITION_ID => (
            PointerType::AssetDefinitionId,
            to_bytes(&canonical_asset_definition(field.as_str()?)?).ok()?,
        ),
        syscalls::SYSCALL_JSON_GET_INT => {
            let frame = IntValueV1::try_new(canonical_json_integer(field)?)
                .ok()?
                .encode_frame()
                .ok()?;
            (PointerType::Int, frame)
        }
        syscalls::SYSCALL_JSON_GET_DECIMAL => {
            let value = canonical_numeric_string::<Numeric>(field)?;
            let frame = DecimalValueV1::from_canonical_numeric(value)
                .ok()?
                .encode_frame()
                .ok()?;
            (PointerType::Decimal, frame)
        }
        syscalls::SYSCALL_JSON_GET_QUANTITY => {
            let frame = QuantityValueV1::new(canonical_numeric_string::<Quantity>(field)?)
                .encode_frame()
                .ok()?;
            (PointerType::Quantity, frame)
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
        Some((pointer_type, payload)) => {
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

    fn quantity_frame(value: Numeric) -> Vec<u8> {
        let quantity = Quantity::from_canonical_numeric(value).expect("canonical quantity");
        QuantityValueV1::new(quantity)
            .encode_frame()
            .expect("quantity frame")
    }

    #[test]
    fn build_json_uses_canonical_number_tokens_across_the_json_integer_domain() {
        let values = [BigInt::from(-7_i64), BigInt::from(u64::MAX)];
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Array { arity: 2 },
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
                &to_bytes(&schema).expect("schema"),
            ))
            .expect("schema TLV");
        let table = vm.alloc_heap(16).expect("word table");
        for (index, value) in values.iter().enumerate() {
            let frame = IntValueV1::try_new(value.clone())
                .expect("int is inside V1 domain")
                .encode_frame()
                .expect("canonical int frame");
            let pointer = vm
                .alloc_input_tlv(&tlv(PointerType::Int, &frame))
                .expect("int TLV");
            vm.store_u64(table + u64::try_from(index).unwrap() * 8, pointer)
                .expect("table word");
        }
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
                njson::Value::from(-7_i64),
                njson::Value::from(u64::MAX),
            ])
        );
    }

    #[test]
    fn build_json_rejects_int_outside_the_native_json_integer_domain() {
        let value = "1606938044258990275541962092341162602522202993782792835301376"
            .parse::<BigInt>()
            .expect("2^200 fits the Kotodama int domain");
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![JsonConstructionNodeV1::Value {
                schema: leaf(StateValueKindV1::Int),
            }],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&schema).expect("schema"),
            ))
            .expect("schema TLV");
        let frame = IntValueV1::try_new(value)
            .expect("int is inside V1 domain")
            .encode_frame()
            .expect("canonical int frame");
        let pointer = vm
            .alloc_input_tlv(&tlv(PointerType::Int, &frame))
            .expect("int TLV");
        let table = vm.alloc_heap(8).expect("word table");
        vm.store_u64(table, pointer).expect("table word");
        vm.set_register(10, schema_ptr);
        vm.set_register(11, table);
        vm.set_register(12, 1);

        assert_eq!(
            build_json(&mut vm, CoreHost::resolve_code_tlv_addr),
            Err(VMError::DecodeError)
        );
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
        let amount_payload = quantity_frame(amount);
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Object {
                    keys: vec!["z_owner".into(), "a_amount".into(), "bytes".into()],
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::AccountId),
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Quantity),
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
            .alloc_input_tlv(&tlv(PointerType::Quantity, &amount_payload))
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
        let amount_payload = quantity_frame(amount);
        let element_schema = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Quantity),
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
            .alloc_input_tlv(&tlv(PointerType::Quantity, &amount_payload))
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
    fn build_json_preserves_full_u64_and_scale_28_quantity_without_floats() {
        let maximum = Numeric::new(u64::MAX, 0);
        let precise = Numeric::new(1_u32, 28);
        let schema = JsonConstructionSchemaV1 {
            nodes: vec![
                JsonConstructionNodeV1::Array { arity: 2 },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Int),
                },
                JsonConstructionNodeV1::Value {
                    schema: leaf(StateValueKindV1::Quantity),
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
                PointerType::Int,
                &IntValueV1::try_new(maximum.mantissa().clone())
                    .expect("maximum test int is inside V1 domain")
                    .encode_frame()
                    .expect("maximum int frame"),
            ))
            .expect("int TLV");
        let precise_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Quantity, &quantity_frame(precise)))
            .expect("quantity TLV");
        let table = vm.alloc_heap(16).expect("word table");
        vm.store_u64(table, maximum_ptr).expect("u128 word");
        vm.store_u64(table + 8, precise_ptr).expect("quantity word");
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
                njson::Value::from(u64::MAX),
                njson::Value::from("0.0000000000000000000000000001"),
            ])
        );
    }

    #[test]
    fn int_getter_accepts_only_actual_json_integer_tokens() {
        assert_eq!(
            canonical_json_integer(&njson::Value::from(-1_i64)),
            Some(BigInt::from(-1_i64))
        );
        assert_eq!(
            canonical_json_integer(&njson::Value::from(u64::MAX)),
            Some(BigInt::from(u64::MAX))
        );
        for value in [
            njson::Value::from("7"),
            njson::Value::from(1.5_f64),
            njson::Value::Bool(true),
            njson::Value::Null,
            njson::Value::Array(Vec::new()),
            njson::Value::Object(njson::Map::new()),
        ] {
            assert_eq!(canonical_json_integer(&value), None);
        }
        let overflow = njson::parse_value("18446744073709551616")
            .expect("generic JSON may represent this token outside the integer domain");
        assert_eq!(
            canonical_json_integer(&overflow),
            None,
            "get_int must reject a numeric token outside its u64 domain"
        );
    }

    #[test]
    fn typed_getter_returns_active_only_some_and_none_handles() {
        let json = Json::from_str_norito(
            r#"{"count":7,"maximum":18446744073709551615,"string":"7","wrong":true}"#,
        )
        .expect("integer getter fixture");
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
            syscalls::SYSCALL_JSON_GET_INT,
            CoreHost::resolve_code_tlv_addr,
        )
        .expect("get count");
        let (some, words) = crate::sum::read_words(
            &vm,
            vm.register(10),
            crate::sum::SumLayoutV1::option(1).unwrap(),
        )
        .expect("read int option");
        assert!(some);
        let int = vm.validate_tlv(words[0]).expect("int TLV");
        assert_eq!(int.type_id, PointerType::Int);
        assert_eq!(
            IntValueV1::decode_frame(int.payload)
                .expect("int frame")
                .into_int(),
            BigInt::from_i128(7)
        );

        let maximum: Name = "maximum".parse().expect("maximum key");
        let maximum_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::Name,
                &to_bytes(&maximum).expect("maximum key"),
            ))
            .expect("maximum key TLV");
        vm.set_register(10, json_ptr);
        vm.set_register(11, maximum_ptr);
        typed_getter(
            &mut vm,
            syscalls::SYSCALL_JSON_GET_INT,
            CoreHost::resolve_code_tlv_addr,
        )
        .expect("get maximum");
        let (some, words) = crate::sum::read_words(
            &vm,
            vm.register(10),
            crate::sum::SumLayoutV1::option(1).unwrap(),
        )
        .expect("read maximum int option");
        assert!(some);
        let int = vm.validate_tlv(words[0]).expect("maximum int TLV");
        assert_eq!(int.type_id, PointerType::Int);
        assert_eq!(
            IntValueV1::decode_frame(int.payload)
                .expect("maximum int frame")
                .into_int(),
            BigInt::from(u64::MAX)
        );

        let missing: Name = "missing".parse().expect("missing key");
        let missing_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Name, &to_bytes(&missing).unwrap()))
            .expect("missing key TLV");
        vm.set_register(10, json_ptr);
        vm.set_register(11, missing_ptr);
        typed_getter(
            &mut vm,
            syscalls::SYSCALL_JSON_GET_INT,
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

        let string: Name = "string".parse().expect("string key");
        let string_ptr = vm
            .alloc_input_tlv(&tlv(PointerType::Name, &to_bytes(&string).unwrap()))
            .expect("string key TLV");
        vm.set_register(10, json_ptr);
        vm.set_register(11, string_ptr);
        typed_getter(
            &mut vm,
            syscalls::SYSCALL_JSON_GET_INT,
            CoreHost::resolve_code_tlv_addr,
        )
        .expect("numeric string is none");
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
            syscalls::SYSCALL_JSON_GET_INT,
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
            syscalls::SYSCALL_JSON_GET_INT,
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
    fn typed_quantity_getter_accepts_only_canonical_string_values() {
        let oversized = "9".repeat(200);
        let json = Json::from(norito::json!({
            "decimal": "1.25",
            "trailing": "1.2500",
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
        let option_layout = crate::sum::SumLayoutV1::option(1).expect("Option<quantity> layout");

        for (key, expected) in [
            ("decimal", Some(Numeric::new(125_u32, 2))),
            ("trailing", None),
            ("integer", None),
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
                syscalls::SYSCALL_JSON_GET_QUANTITY,
                CoreHost::resolve_code_tlv_addr,
            )
            .expect("typed quantity getter");
            let (some, payload) = crate::sum::read_words(&vm, vm.register(10), option_layout)
                .expect("read Option<quantity>");
            match expected {
                Some(expected) => {
                    assert!(some, "{key} must produce Option::some");
                    let amount = vm.validate_tlv(payload[0]).expect("quantity TLV");
                    assert_eq!(amount.type_id, PointerType::Quantity);
                    let amount = QuantityValueV1::decode_frame(amount.payload)
                        .expect("decode canonical quantity")
                        .into_quantity();
                    assert_eq!(amount.as_numeric(), &expected);
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
    fn build_json_rejects_noncanonical_quantities_and_hidden_option_payloads() {
        let quantity_schema = JsonConstructionSchemaV1 {
            nodes: vec![JsonConstructionNodeV1::Value {
                schema: leaf(StateValueKindV1::Quantity),
            }],
        };
        let mut vm = IVM::new(u64::MAX);
        let quantity_schema_ptr = vm
            .alloc_input_tlv(&tlv(
                PointerType::NoritoBytes,
                &to_bytes(&quantity_schema).expect("quantity schema"),
            ))
            .expect("quantity schema TLV");
        let noncanonical = vm
            .alloc_input_tlv(&tlv(
                PointerType::Quantity,
                &to_bytes(&Numeric::new(10_u32, 1)).expect("noncanonical quantity payload"),
            ))
            .expect("noncanonical quantity TLV");
        let quantity_table = vm.alloc_heap(8).expect("quantity word table");
        vm.store_u64(quantity_table, noncanonical)
            .expect("store quantity pointer");
        vm.set_register(10, quantity_schema_ptr);
        vm.set_register(11, quantity_table);
        vm.set_register(12, 1);
        assert_eq!(
            build_json(&mut vm, CoreHost::resolve_code_tlv_addr),
            Err(VMError::DecodeError),
            "quantity inputs must already use their unique canonical frame",
        );

        let option_schema = JsonConstructionSchemaV1 {
            nodes: vec![JsonConstructionNodeV1::Value {
                schema: StateValueSchemaV1 {
                    nodes: vec![
                        StateValueNodeV1::Option,
                        StateValueNodeV1::Leaf(StateValueKindV1::Quantity),
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
        let option_layout = crate::sum::SumLayoutV1::option(1).expect("Option<quantity> layout");
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
                syscalls::SYSCALL_JSON_GET_INT,
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
                syscalls::SYSCALL_JSON_GET_INT,
                CoreHost::resolve_code_tlv_addr,
            ),
            Err(VMError::DecodeError)
        );
    }
}
