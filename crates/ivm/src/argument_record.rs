//! One-shot decoding for compiler-generated public entrypoint wrappers.
//!
//! Torii, CLI, and SDK boundaries may accept ergonomic JSON, but they convert
//! it before signing into one schema-hashed canonical Norito record. Execution
//! preparation decodes and validates that complete record once. Contract
//! wrappers still supply their compact schema, while the host only verifies the
//! binding and materializes a VM-owned table of ABI words in declaration order.

use std::{mem::size_of, str::FromStr, sync::Arc};

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    prelude::{AssetDefinitionId, AssetId, DataSpaceId, DomainId, Name, NftId},
};
use iroha_primitives::{json::Json, numeric::Numeric};
use ivm_abi::entrypoint::{
    EntrypointArgumentRecordV1, EntrypointArgumentSchemaV1, EntrypointValueAtomV1,
    EntrypointValueKindV1, EntrypointValueTypeNodeV1, EntrypointValueTypeV1,
    EntrypointValueWordKindV1, MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES,
    MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES, MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES,
    MAX_ENTRYPOINT_ARGUMENT_WORDS, MAX_ENTRYPOINT_ARGUMENTS, entrypoint_argument_schema_hash_v1,
    entrypoint_value_subtree_range_v1,
};
use ivm_abi::list::ListLayoutV1;
use ivm_abi::sum::SumLayoutV1;
use norito::{NoritoSerialize, decode_from_bytes, json as njson, to_bytes};

use crate::{
    VMError,
    host::quote_tlv_payload_len_at,
    ivm::IVM,
    pointer_abi::{self, PointerType, Tlv},
};

const ARGUMENT_DECODE_GAS_BASE: u64 = 32;
const ARGUMENT_DECODE_GAS_PER_BYTE: u64 = 1;
const TLV_ENVELOPE_BYTES: usize = 7 + Hash::LENGTH;
const ARGUMENT_RECORD_BINDING_DOMAIN_V1: &[u8] = b"iroha:ivm:argument-record-binding:v1";

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
        RECORD_DECODE_COUNT.with(std::cell::Cell::get)
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
    Sum {
        layout: SumLayoutV1,
        tag: u64,
        active: Vec<usize>,
    },
    List {
        layout: ListLayoutV1,
        elements: Vec<Vec<usize>>,
    },
}

#[derive(Debug)]
struct ArgumentDecodePlan {
    decoded: Vec<DecodedArgument>,
    roots: Vec<usize>,
    record_bytes: usize,
    schema_bytes: usize,
    materialized_bytes: u64,
}

impl ArgumentDecodePlan {
    fn gas(&self) -> u64 {
        argument_record_gas_for_bytes(
            self.record_bytes,
            self.schema_bytes,
            self.materialized_bytes,
        )
    }

    fn allocation_lengths(&self) -> Vec<usize> {
        let mut lengths = Vec::new();
        for value in &self.decoded {
            if let DecodedArgument::Pointer(envelope) = value {
                lengths.push(envelope.len());
            }
        }
        lengths.push(decoded_table_envelope_len(self.roots.len()));
        lengths
    }

    fn raw_heap_bytes(&self) -> u64 {
        self.decoded
            .iter()
            .map(|value| match value {
                DecodedArgument::Scalar(_) | DecodedArgument::Pointer(_) => 0,
                DecodedArgument::Sum { layout, .. } => {
                    layout.allocation_bytes().unwrap_or(u64::MAX)
                }
                DecodedArgument::List { layout, .. } => {
                    layout.allocation_bytes().unwrap_or(u64::MAX)
                }
            })
            .fold(0, u64::saturating_add)
    }
}

struct PreparedArgumentRecordInner {
    canonical_record: Arc<[u8]>,
    canonical_schema: Arc<[u8]>,
    binding: [u8; Hash::LENGTH],
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
            .field("abi_words", &self.inner.decode_plan.roots.len())
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

    /// Return the domain-separated capability payload exposed to guest code.
    ///
    /// The full signed record remains host-owned; compiler-generated wrappers
    /// receive only this immutable binding and the decoded ABI word table.
    #[must_use]
    pub fn binding_bytes(&self) -> &[u8; Hash::LENGTH] {
        &self.inner.binding
    }

    /// Debit the complete deterministic decode/materialization cost before
    /// guest execution begins.
    ///
    /// This escrow is mandatory for prepared records: it prevents a hand-built
    /// artifact from avoiding argument-decoding gas by branching around the
    /// decode syscall after the host has already prepared the signed record.
    ///
    /// # Errors
    ///
    /// Returns [`VMError::OutOfGas`] when the invocation cannot afford the
    /// prepared record, or [`VMError::DecodeError`] if the same VM is charged
    /// more than once.
    pub fn precharge_vm(&self, vm: &mut IVM) -> Result<(), VMError> {
        vm.prepay_argument_decode(self.inner.decode_plan.gas())
    }

    /// Quote materialization after verifying the VM still presents the record
    /// pointer issued by the host and correctly bounded envelope shapes.
    ///
    /// Payload authentication and exact binding checks happen in
    /// [`Self::install_into_vm`] after the decode/materialization charge has
    /// already been prepaid. The quote path deliberately does not hash or
    /// deserialize attacker-controlled payloads.
    ///
    /// # Errors
    ///
    /// Returns an error if either VM pointer is invalid, substituted, or not
    /// allowed by the active ABI policy, or if the exact decode cost was not
    /// prepaid for this invocation.
    pub fn decode_gas_quote(&self, vm: &IVM, record_pointer: u64) -> Result<u64, VMError> {
        if vm.register(10) != record_pointer {
            return Err(VMError::DecodeError);
        }
        let (record_bytes, schema_bytes) = quote_argument_envelope_lengths(vm)?;
        if record_bytes != self.binding_bytes().len() || schema_bytes != self.schema_bytes().len() {
            return Err(VMError::DecodeError);
        }
        if !vm.argument_decode_is_prepaid(self.inner.decode_plan.gas()) {
            return Err(VMError::DecodeError);
        }
        Ok(0)
    }

    /// Materialize the prepared ABI-word table in VM-owned memory.
    ///
    /// This validates the host-issued record pointer and canonical schema again
    /// for direct host calls, then allocates typed pointer values and the packed
    /// word table without decoding either Norito payload. Allocations prefer
    /// INPUT and spill into the owned HEAP prefix when INPUT cannot fit them.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM inputs do not match this prepared record, the
    /// exact decode cost was not prepaid, or VM-owned INPUT and HEAP capacity
    /// cannot hold the complete allocation sequence.
    pub fn install_into_vm(&self, vm: &mut IVM, record_pointer: u64) -> Result<u64, VMError> {
        self.validate_vm_binding(vm, record_pointer)?;
        vm.consume_prepaid_argument_decode(self.inner.decode_plan.gas())?;
        materialize_decode_plan(vm, &self.inner.decode_plan)?;
        Ok(0)
    }

    fn validate_vm_binding(&self, vm: &IVM, record_pointer: u64) -> Result<(), VMError> {
        if vm.register(10) != record_pointer {
            return Err(VMError::DecodeError);
        }
        let record_tlv = validate_tlv_any_region(vm, record_pointer, PointerType::NoritoBytes)?;
        let schema_tlv = validate_tlv_any_region(vm, vm.register(11), PointerType::NoritoBytes)?;
        validate_argument_envelope_lengths(&record_tlv, &schema_tlv)?;
        if record_tlv.payload != self.binding_bytes() || schema_tlv.payload != self.schema_bytes() {
            return Err(VMError::DecodeError);
        }
        Ok(())
    }
}

fn argument_record_gas_for_bytes(
    record_bytes: usize,
    schema_bytes: usize,
    materialized_bytes: u64,
) -> u64 {
    let charged_bytes = u64::try_from(record_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(u64::try_from(schema_bytes).unwrap_or(u64::MAX))
        .saturating_add(materialized_bytes);
    ARGUMENT_DECODE_GAS_BASE
        .saturating_add(ARGUMENT_DECODE_GAS_PER_BYTE.saturating_mul(charged_bytes))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SchemaMaterializationBound {
    words: u64,
    pointer_envelopes: u64,
    raw_heap_bytes: u64,
}

impl SchemaMaterializationBound {
    const ZERO: Self = Self {
        words: 0,
        pointer_envelopes: 0,
        raw_heap_bytes: 0,
    };
}

fn max_schema_pointer_envelopes() -> u64 {
    u64::try_from(MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES / TLV_ENVELOPE_BYTES).unwrap_or(u64::MAX)
}

fn cap_pointer_envelopes(count: u64) -> u64 {
    count.min(max_schema_pointer_envelopes())
}

fn add_pointer_envelopes(lhs: u64, rhs: u64) -> u64 {
    cap_pointer_envelopes(lhs.saturating_add(rhs))
}

fn multiply_pointer_envelopes(count: u64, multiplier: u64) -> u64 {
    cap_pointer_envelopes(count.saturating_mul(multiplier))
}

fn cap_raw_heap_bytes(bytes: u64) -> u64 {
    bytes.min(crate::memory::Memory::HEAP_SIZE)
}

fn add_raw_heap_bytes(lhs: u64, rhs: u64) -> u64 {
    cap_raw_heap_bytes(lhs.saturating_add(rhs))
}

fn multiply_raw_heap_bytes(bytes: u64, count: u64) -> u64 {
    cap_raw_heap_bytes(bytes.saturating_mul(count))
}

fn aggregate_allocation_bytes(header_words: u64, payload_words: u64) -> u64 {
    cap_raw_heap_bytes(
        header_words
            .saturating_add(payload_words)
            .saturating_mul(u64::try_from(size_of::<u64>()).unwrap_or(u64::MAX)),
    )
}

/// Derive the largest materializable aggregate layout from one validated flat
/// preorder type tape without recursion or attacker-controlled allocation.
fn value_materialization_bound(
    ty: &EntrypointValueTypeV1,
) -> Result<SchemaMaterializationBound, VMError> {
    // The public schema validator enforces this same node bound. A fixed local
    // stack keeps quote construction allocation-free after validation.
    let mut rendered = [SchemaMaterializationBound::ZERO; MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES];
    let mut rendered_len = 0_usize;

    for node in ty.nodes.iter().rev() {
        let child_count = argument_node_child_count(node);
        let children_start = rendered_len
            .checked_sub(child_count)
            .ok_or(VMError::DecodeError)?;
        let children = &rendered[children_start..rendered_len];
        let bound = match node {
            EntrypointValueTypeNodeV1::Struct(_) | EntrypointValueTypeNodeV1::Tuple(_) => children
                .iter()
                .fold(SchemaMaterializationBound::ZERO, |total, child| {
                    SchemaMaterializationBound {
                        words: total.words.saturating_add(child.words),
                        pointer_envelopes: add_pointer_envelopes(
                            total.pointer_envelopes,
                            child.pointer_envelopes,
                        ),
                        raw_heap_bytes: add_raw_heap_bytes(
                            total.raw_heap_bytes,
                            child.raw_heap_bytes,
                        ),
                    }
                }),
            EntrypointValueTypeNodeV1::Option => {
                let child = *children.first().ok_or(VMError::DecodeError)?;
                let own_allocation = aggregate_allocation_bytes(1, child.words);
                SchemaMaterializationBound {
                    words: 1,
                    pointer_envelopes: child.pointer_envelopes,
                    raw_heap_bytes: add_raw_heap_bytes(own_allocation, child.raw_heap_bytes),
                }
            }
            EntrypointValueTypeNodeV1::Result => {
                let first = *children.first().ok_or(VMError::DecodeError)?;
                let second = *children.get(1).ok_or(VMError::DecodeError)?;
                let own_allocation = aggregate_allocation_bytes(1, first.words.max(second.words));
                SchemaMaterializationBound {
                    words: 1,
                    pointer_envelopes: first.pointer_envelopes.max(second.pointer_envelopes),
                    raw_heap_bytes: add_raw_heap_bytes(
                        own_allocation,
                        first.raw_heap_bytes.max(second.raw_heap_bytes),
                    ),
                }
            }
            EntrypointValueTypeNodeV1::List(list) => {
                let element = *children.first().ok_or(VMError::DecodeError)?;
                let capacity = u64::from(list.capacity);
                let own_allocation =
                    aggregate_allocation_bytes(2, capacity.saturating_mul(element.words));
                SchemaMaterializationBound {
                    words: 1,
                    pointer_envelopes: multiply_pointer_envelopes(
                        element.pointer_envelopes,
                        capacity,
                    ),
                    raw_heap_bytes: add_raw_heap_bytes(
                        own_allocation,
                        multiply_raw_heap_bytes(element.raw_heap_bytes, capacity),
                    ),
                }
            }
            EntrypointValueTypeNodeV1::Leaf(kind) => SchemaMaterializationBound {
                words: 1,
                pointer_envelopes: u64::from(kind.is_pointer()),
                raw_heap_bytes: 0,
            },
        };

        rendered_len = children_start;
        let slot = rendered.get_mut(rendered_len).ok_or(VMError::DecodeError)?;
        *slot = bound;
        rendered_len = rendered_len.checked_add(1).ok_or(VMError::DecodeError)?;
    }

    if rendered_len != 1 {
        return Err(VMError::DecodeError);
    }
    Ok(rendered[0])
}

fn schema_materialization_bound(
    schema: &EntrypointArgumentSchemaV1,
) -> Result<SchemaMaterializationBound, VMError> {
    if !schema.validate() {
        return Err(VMError::DecodeError);
    }

    let bound =
        schema
            .fields
            .iter()
            .try_fold(SchemaMaterializationBound::ZERO, |total, field| {
                let field = value_materialization_bound(&field.ty)?;
                Ok::<_, VMError>(SchemaMaterializationBound {
                    words: total.words.saturating_add(field.words),
                    pointer_envelopes: add_pointer_envelopes(
                        total.pointer_envelopes,
                        field.pointer_envelopes,
                    ),
                    raw_heap_bytes: add_raw_heap_bytes(total.raw_heap_bytes, field.raw_heap_bytes),
                })
            })?;
    if usize::try_from(bound.words).ok() != schema.word_count()
        || bound.words > u64::try_from(MAX_ENTRYPOINT_ARGUMENT_WORDS).unwrap_or(u64::MAX)
    {
        return Err(VMError::DecodeError);
    }
    Ok(bound)
}

fn argument_record_gas_for_schema_bound(
    record_bytes: usize,
    schema_bytes: usize,
    bound: SchemaMaterializationBound,
) -> u64 {
    argument_record_gas_for_bytes(
        record_bytes,
        schema_bytes,
        materialized_bytes_for_schema_bound(record_bytes, bound),
    )
}

fn aligned_allocation_bytes(length: usize) -> u64 {
    u64::try_from(length)
        .ok()
        .and_then(|length| length.checked_add(7))
        .map(|length| length & !7)
        .unwrap_or(u64::MAX)
}

fn pointer_copy_allocation_upper_bound(record_bytes: usize, pointer_envelopes: u64) -> u64 {
    let record_bytes = u64::try_from(record_bytes).unwrap_or(u64::MAX);
    let envelope_bytes = u64::try_from(TLV_ENVELOPE_BYTES).unwrap_or(u64::MAX);
    let record_pointer_limit = record_bytes.checked_div(envelope_bytes).unwrap_or(0);
    record_bytes.saturating_add(
        pointer_envelopes
            .min(record_pointer_limit)
            .saturating_mul(7),
    )
}

fn materialized_bytes_for_schema_bound(
    record_bytes: usize,
    bound: SchemaMaterializationBound,
) -> u64 {
    let word_count = usize::try_from(bound.words).unwrap_or(usize::MAX);
    pointer_copy_allocation_upper_bound(record_bytes, bound.pointer_envelopes)
        .saturating_add(aligned_allocation_bytes(decoded_table_envelope_len(
            word_count,
        )))
        .saturating_add(bound.raw_heap_bytes)
}

fn argument_record_runtime_gas_upper_bound(record_bytes: usize, schema_bytes: usize) -> u64 {
    // Every pointer envelope is embedded in the record, so one additional
    // record-sized allowance bounds all VM copies before their shapes are
    // decoded. The raw syscall cannot authenticate or parse its schema until
    // after gas debit, so it deliberately retains the full-HEAP reserve.
    let materialized_bytes = pointer_copy_allocation_upper_bound(record_bytes, u64::MAX)
        .saturating_add(aligned_allocation_bytes(decoded_table_envelope_len(
            MAX_ENTRYPOINT_ARGUMENT_WORDS,
        )))
        .saturating_add(crate::memory::Memory::HEAP_SIZE);
    argument_record_gas_for_bytes(record_bytes, schema_bytes, materialized_bytes)
}

fn canonical_norito_frame_len<T: NoritoSerialize>(value: &T) -> usize {
    let payload_len = norito::codec::Encode::encoded_len(value);
    let alignment = core::mem::align_of::<norito::Archived<T>>();
    let remainder = norito::core::Header::SIZE % alignment;
    let padding = if remainder == 0 {
        0
    } else {
        alignment - remainder
    };
    norito::core::Header::SIZE
        .saturating_add(padding)
        .saturating_add(payload_len)
}

fn decoded_table_envelope_len(word_count: usize) -> usize {
    TLV_ENVELOPE_BYTES
        .saturating_add(1)
        .saturating_add(word_count.saturating_mul(size_of::<u64>()))
}

fn argument_record_binding(canonical_record: &[u8]) -> [u8; Hash::LENGTH] {
    let record_hash = Hash::new(canonical_record);
    let mut material = Vec::with_capacity(
        ARGUMENT_RECORD_BINDING_DOMAIN_V1.len() + size_of::<u64>() + Hash::LENGTH,
    );
    material.extend_from_slice(ARGUMENT_RECORD_BINDING_DOMAIN_V1);
    material.extend_from_slice(
        &u64::try_from(canonical_record.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    material.extend_from_slice(record_hash.as_ref());
    Hash::new(&material).into()
}

fn validate_tlv_any_region(
    vm: &IVM,
    address: u64,
    expected: PointerType,
) -> Result<Tlv<'_>, VMError> {
    vm.ensure_owned_public_tlv_range(address, 7)?;
    let header = vm
        .memory
        .load_region(address, 7)
        .map_err(|_| VMError::NoritoInvalid)?;
    let payload_len = u32::from_be_bytes([header[3], header[4], header[5], header[6]]) as usize;
    let envelope_len = 7usize
        .checked_add(payload_len)
        .and_then(|len| len.checked_add(Hash::LENGTH))
        .ok_or(VMError::NoritoInvalid)?;
    vm.ensure_owned_public_tlv_range(
        address,
        u64::try_from(envelope_len).map_err(|_| VMError::NoritoInvalid)?,
    )?;
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
    kind: &EntrypointValueKindV1,
    value: &njson::Value,
) -> Result<EntrypointValueAtomV1, VMError> {
    let encoded_pointer = |pointer_type, payload: Vec<u8>| {
        encode_tlv(pointer_type, &payload).map(EntrypointValueAtomV1::Pointer)
    };
    Ok(match kind {
        EntrypointValueKindV1::Int => EntrypointValueAtomV1::Int(decode_i64(value)?),
        EntrypointValueKindV1::U128 => encoded_pointer(
            PointerType::NoritoBytes,
            to_bytes(&decode_u128(value)?).map_err(|_| VMError::NoritoInvalid)?,
        )?,
        EntrypointValueKindV1::Bool => {
            EntrypointValueAtomV1::Bool(value.as_bool().ok_or(VMError::DecodeError)?)
        }
        EntrypointValueKindV1::String => encoded_pointer(
            PointerType::Blob,
            value
                .as_str()
                .ok_or(VMError::DecodeError)?
                .as_bytes()
                .to_vec(),
        )?,
        EntrypointValueKindV1::Amount => {
            let amount = decode_numeric(value)?
                .canonicalize_amount()
                .map_err(|_| VMError::DecodeError)?;
            encoded_pointer(
                PointerType::Amount,
                to_bytes(&amount).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::Json => encoded_pointer(
            PointerType::Json,
            to_bytes(&Json::from_norito_value_ref(value).map_err(|_| VMError::DecodeError)?)
                .map_err(|_| VMError::NoritoInvalid)?,
        )?,
        EntrypointValueKindV1::Name => {
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
        EntrypointValueKindV1::AccountId => {
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
        EntrypointValueKindV1::AssetDefinitionId => {
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
        EntrypointValueKindV1::AssetId => {
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
        EntrypointValueKindV1::DomainId => {
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
        EntrypointValueKindV1::NftId => {
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
        EntrypointValueKindV1::DataSpaceId => {
            let value = DataSpaceId::new(decode_u64(value)?);
            encoded_pointer(
                PointerType::DataSpaceId,
                to_bytes(&value).map_err(|_| VMError::NoritoInvalid)?,
            )?
        }
        EntrypointValueKindV1::Blob => encoded_pointer(PointerType::Blob, decode_blob(value)?)?,
    })
}

fn argument_node_child_count(node: &EntrypointValueTypeNodeV1) -> usize {
    match node {
        EntrypointValueTypeNodeV1::Struct(node) => node.fields.len(),
        EntrypointValueTypeNodeV1::Tuple(arity) => usize::from(*arity),
        EntrypointValueTypeNodeV1::Option | EntrypointValueTypeNodeV1::List(_) => 1,
        EntrypointValueTypeNodeV1::Result => 2,
        EntrypointValueTypeNodeV1::Leaf(_) => 0,
    }
}

/// Return the exclusive end of one preorder subtree without recursion.
///
/// The data-model walker is the authoritative structural cursor. All aggregate
/// children, including a List's element type, live inline in the same tape.
fn argument_subtree_end(
    nodes: &[EntrypointValueTypeNodeV1],
    start: usize,
) -> Result<usize, VMError> {
    entrypoint_value_subtree_range_v1(nodes, start)
        .map(|range| range.end)
        .ok_or(VMError::DecodeError)
}

fn argument_child_starts(
    nodes: &[EntrypointValueTypeNodeV1],
    node_start: usize,
    child_count: usize,
) -> Result<Vec<usize>, VMError> {
    let mut child = node_start.checked_add(1).ok_or(VMError::DecodeError)?;
    let mut starts = Vec::with_capacity(child_count);
    for _ in 0..child_count {
        starts.push(child);
        child = argument_subtree_end(nodes, child)?;
    }
    if child != argument_subtree_end(nodes, node_start)? {
        return Err(VMError::DecodeError);
    }
    Ok(starts)
}

fn argument_node_word_count(
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
) -> Result<usize, VMError> {
    #[derive(Clone, Copy)]
    struct Frame {
        remaining: usize,
        suppress_words: bool,
    }

    let start = *node_index;
    let end = argument_subtree_end(nodes, start)?;
    let mut frames = Vec::<Frame>::new();
    let mut words = 0_usize;
    for (offset, node) in nodes[start..end].iter().enumerate() {
        while frames.last().is_some_and(|frame| frame.remaining == 0) {
            frames.pop();
        }
        let suppress_words = if offset == 0 {
            false
        } else {
            let parent = frames.last_mut().ok_or(VMError::DecodeError)?;
            parent.remaining = parent
                .remaining
                .checked_sub(1)
                .ok_or(VMError::DecodeError)?;
            parent.suppress_words
        };
        let is_handle = matches!(
            node,
            EntrypointValueTypeNodeV1::Option
                | EntrypointValueTypeNodeV1::Result
                | EntrypointValueTypeNodeV1::List(_)
        );
        if !suppress_words && (is_handle || matches!(node, EntrypointValueTypeNodeV1::Leaf(_))) {
            words = words.checked_add(1).ok_or(VMError::DecodeError)?;
        }
        let children = argument_node_child_count(node);
        if children != 0 {
            frames.push(Frame {
                remaining: children,
                suppress_words: suppress_words || is_handle,
            });
        }
    }
    while frames.last().is_some_and(|frame| frame.remaining == 0) {
        frames.pop();
    }
    if !frames.is_empty() {
        return Err(VMError::DecodeError);
    }
    *node_index = end;
    Ok(words)
}

fn decode_argument_node(
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
    value: &njson::Value,
    out: &mut Vec<EntrypointValueAtomV1>,
) -> Result<(), VMError> {
    enum Task<'a> {
        Visit {
            node_start: usize,
            value: &'a njson::Value,
        },
        FinishProduct {
            children: usize,
        },
        FinishSum {
            tag: bool,
        },
        FinishList {
            items: usize,
        },
    }

    let start = *node_index;
    let end = argument_subtree_end(nodes, start)?;
    let mut tasks = vec![Task::Visit {
        node_start: start,
        value,
    }];
    let mut results = Vec::<Vec<EntrypointValueAtomV1>>::new();

    while let Some(task) = tasks.pop() {
        match task {
            Task::Visit { node_start, value } => {
                let node = nodes.get(node_start).ok_or(VMError::DecodeError)?;
                match node {
                    EntrypointValueTypeNodeV1::Struct(node) => {
                        let object = value.as_object().ok_or(VMError::DecodeError)?;
                        if object.len() != node.fields.len() {
                            return Err(VMError::DecodeError);
                        }
                        let starts = argument_child_starts(nodes, node_start, node.fields.len())?;
                        tasks.push(Task::FinishProduct {
                            children: starts.len(),
                        });
                        for (child, field) in starts.iter().zip(&node.fields).rev() {
                            tasks.push(Task::Visit {
                                node_start: *child,
                                value: object.get(field).ok_or(VMError::DecodeError)?,
                            });
                        }
                    }
                    EntrypointValueTypeNodeV1::Tuple(arity) => {
                        let values = value.as_array().ok_or(VMError::DecodeError)?;
                        if values.len() != usize::from(*arity) {
                            return Err(VMError::DecodeError);
                        }
                        let starts = argument_child_starts(nodes, node_start, values.len())?;
                        tasks.push(Task::FinishProduct {
                            children: starts.len(),
                        });
                        for (child, value) in starts.iter().zip(values).rev() {
                            tasks.push(Task::Visit {
                                node_start: *child,
                                value,
                            });
                        }
                    }
                    EntrypointValueTypeNodeV1::Option => {
                        let object = value.as_object().ok_or(VMError::DecodeError)?;
                        if object.len() != 1 {
                            return Err(VMError::DecodeError);
                        }
                        if let Some(value) = object.get("some") {
                            tasks.push(Task::FinishSum { tag: true });
                            tasks.push(Task::Visit {
                                node_start: node_start
                                    .checked_add(1)
                                    .ok_or(VMError::DecodeError)?,
                                value,
                            });
                        } else if object.get("none") == Some(&njson::Value::Bool(true)) {
                            results.push(vec![EntrypointValueAtomV1::Tag(false)]);
                        } else {
                            return Err(VMError::DecodeError);
                        }
                    }
                    EntrypointValueTypeNodeV1::Result => {
                        let object = value.as_object().ok_or(VMError::DecodeError)?;
                        if object.len() != 1 {
                            return Err(VMError::DecodeError);
                        }
                        let ok_start = node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                        let err_start = argument_subtree_end(nodes, ok_start)?;
                        if let Some(value) = object.get("ok") {
                            tasks.push(Task::FinishSum { tag: true });
                            tasks.push(Task::Visit {
                                node_start: ok_start,
                                value,
                            });
                        } else if let Some(value) = object.get("err") {
                            tasks.push(Task::FinishSum { tag: false });
                            tasks.push(Task::Visit {
                                node_start: err_start,
                                value,
                            });
                        } else {
                            return Err(VMError::DecodeError);
                        }
                    }
                    EntrypointValueTypeNodeV1::List(list) => {
                        let values = value.as_array().ok_or(VMError::DecodeError)?;
                        if values.len() > usize::from(list.capacity) {
                            return Err(VMError::DecodeError);
                        }
                        let element_start =
                            node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                        let _ = argument_subtree_end(nodes, element_start)?;
                        tasks.push(Task::FinishList {
                            items: values.len(),
                        });
                        for value in values.iter().rev() {
                            tasks.push(Task::Visit {
                                node_start: element_start,
                                value,
                            });
                        }
                    }
                    EntrypointValueTypeNodeV1::Leaf(kind) => {
                        results.push(vec![encode_leaf_atom(kind, value)?]);
                    }
                }
            }
            Task::FinishProduct { children } => {
                let split = results
                    .len()
                    .checked_sub(children)
                    .ok_or(VMError::DecodeError)?;
                let children = results.split_off(split);
                let capacity = children
                    .iter()
                    .try_fold(0_usize, |total, child| total.checked_add(child.len()))
                    .ok_or(VMError::DecodeError)?;
                let mut product = Vec::with_capacity(capacity);
                for child in children {
                    product.extend(child);
                }
                results.push(product);
            }
            Task::FinishSum { tag } => {
                let child = results.pop().ok_or(VMError::DecodeError)?;
                let mut sum = Vec::with_capacity(child.len().saturating_add(1));
                sum.push(EntrypointValueAtomV1::Tag(tag));
                sum.extend(child);
                results.push(sum);
            }
            Task::FinishList { items } => {
                let split = results
                    .len()
                    .checked_sub(items)
                    .ok_or(VMError::DecodeError)?;
                let items = results.split_off(split);
                let item_count = u8::try_from(items.len()).map_err(|_| VMError::DecodeError)?;
                let capacity = items
                    .iter()
                    .try_fold(1_usize, |total, item| total.checked_add(item.len()))
                    .ok_or(VMError::DecodeError)?;
                let mut list = Vec::with_capacity(capacity);
                list.push(EntrypointValueAtomV1::List(item_count));
                for item in items {
                    list.extend(item);
                }
                results.push(list);
            }
        }
    }

    if results.len() != 1 {
        return Err(VMError::DecodeError);
    }
    out.extend(results.pop().expect("length checked"));
    *node_index = end;
    Ok(())
}

fn decode_argument_value(
    ty: &EntrypointValueTypeV1,
    value: &njson::Value,
    out: &mut Vec<EntrypointValueAtomV1>,
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
    if !schema.validate_atoms(&atoms) {
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
    validate_argument_envelope_payload_lengths(record.payload.len(), schema.payload.len())
}

fn validate_argument_envelope_payload_lengths(
    record_bytes: usize,
    schema_bytes: usize,
) -> Result<(), VMError> {
    if record_bytes > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES
        || schema_bytes > MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES
    {
        return Err(VMError::NoritoInvalid);
    }
    Ok(())
}

fn quote_argument_envelope_lengths(vm: &IVM) -> Result<(usize, usize), VMError> {
    let record_bytes = quote_tlv_payload_len_at(vm, vm.register(10), PointerType::NoritoBytes)?;
    let schema_bytes = quote_tlv_payload_len_at(vm, vm.register(11), PointerType::NoritoBytes)?;
    validate_argument_envelope_payload_lengths(record_bytes, schema_bytes)?;
    Ok((record_bytes, schema_bytes))
}

fn expected_pointer_type(kind: EntrypointValueKindV1) -> Option<PointerType> {
    Some(match kind {
        EntrypointValueKindV1::Int | EntrypointValueKindV1::Bool => return None,
        EntrypointValueKindV1::U128 => PointerType::NoritoBytes,
        EntrypointValueKindV1::Amount => PointerType::Amount,
        EntrypointValueKindV1::String | EntrypointValueKindV1::Blob => PointerType::Blob,
        EntrypointValueKindV1::Json => PointerType::Json,
        EntrypointValueKindV1::Name => PointerType::Name,
        EntrypointValueKindV1::AccountId => PointerType::AccountId,
        EntrypointValueKindV1::AssetDefinitionId => PointerType::AssetDefinitionId,
        EntrypointValueKindV1::AssetId => PointerType::AssetId,
        EntrypointValueKindV1::DomainId => PointerType::DomainId,
        EntrypointValueKindV1::NftId => PointerType::NftId,
        EntrypointValueKindV1::DataSpaceId => PointerType::DataSpaceId,
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

fn validate_pointer_payload(kind: EntrypointValueKindV1, payload: &[u8]) -> Result<(), VMError> {
    match kind {
        EntrypointValueKindV1::Int | EntrypointValueKindV1::Bool => {
            return Err(VMError::DecodeError);
        }
        EntrypointValueKindV1::U128 => {
            let value: Numeric = decode_canonical_norito(payload)?;
            if value.scale() != 0 || value.try_mantissa_u128().is_none() {
                return Err(VMError::DecodeError);
            }
        }
        EntrypointValueKindV1::Amount => {
            let value: Numeric = decode_canonical_norito(payload)?;
            value.validate_amount().map_err(|_| VMError::DecodeError)?;
        }
        EntrypointValueKindV1::String => {
            std::str::from_utf8(payload).map_err(|_| VMError::DecodeError)?;
        }
        EntrypointValueKindV1::Json => {
            let _: Json = decode_canonical_norito(payload)?;
        }
        EntrypointValueKindV1::Name => {
            let _: Name = decode_canonical_norito(payload)?;
        }
        EntrypointValueKindV1::AccountId => {
            let _: AccountId = decode_canonical_norito(payload)?;
        }
        EntrypointValueKindV1::AssetDefinitionId => {
            let _: AssetDefinitionId = decode_canonical_norito(payload)?;
        }
        EntrypointValueKindV1::AssetId => {
            let _: AssetId = decode_canonical_norito(payload)?;
        }
        EntrypointValueKindV1::DomainId => {
            let _: DomainId = decode_canonical_norito(payload)?;
        }
        EntrypointValueKindV1::NftId => {
            let _: NftId = decode_canonical_norito(payload)?;
        }
        EntrypointValueKindV1::DataSpaceId => {
            let _: DataSpaceId = decode_canonical_norito(payload)?;
        }
        EntrypointValueKindV1::Blob => {}
    }
    Ok(())
}

fn validate_pointer_atom(
    policy: ivm_abi::SyscallPolicy,
    kind: EntrypointValueKindV1,
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

fn validate_argument_atoms(
    policy: ivm_abi::SyscallPolicy,
    nodes: &[EntrypointValueTypeNodeV1],
    atoms: &[EntrypointValueAtomV1],
    node_index: &mut usize,
    atom_index: &mut usize,
) -> Result<(), VMError> {
    let start = *node_index;
    let end = argument_subtree_end(nodes, start)?;
    let mut actions = vec![start];
    let mut cursor = *atom_index;

    while let Some(node_start) = actions.pop() {
        let node = nodes.get(node_start).ok_or(VMError::DecodeError)?;
        match node {
            EntrypointValueTypeNodeV1::Struct(node) => {
                let starts = argument_child_starts(nodes, node_start, node.fields.len())?;
                actions.extend(starts.into_iter().rev());
            }
            EntrypointValueTypeNodeV1::Tuple(arity) => {
                let starts = argument_child_starts(nodes, node_start, usize::from(*arity))?;
                actions.extend(starts.into_iter().rev());
            }
            EntrypointValueTypeNodeV1::Option => {
                let EntrypointValueAtomV1::Tag(active) =
                    atoms.get(cursor).ok_or(VMError::DecodeError)?
                else {
                    return Err(VMError::DecodeError);
                };
                cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                if *active {
                    actions.push(node_start.checked_add(1).ok_or(VMError::DecodeError)?);
                }
            }
            EntrypointValueTypeNodeV1::Result => {
                let EntrypointValueAtomV1::Tag(ok_active) =
                    atoms.get(cursor).ok_or(VMError::DecodeError)?
                else {
                    return Err(VMError::DecodeError);
                };
                cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                let ok_start = node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                let err_start = argument_subtree_end(nodes, ok_start)?;
                actions.push(if *ok_active { ok_start } else { err_start });
            }
            EntrypointValueTypeNodeV1::List(list) => {
                let EntrypointValueAtomV1::List(item_count) =
                    atoms.get(cursor).ok_or(VMError::DecodeError)?
                else {
                    return Err(VMError::DecodeError);
                };
                cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                if *item_count > list.capacity {
                    return Err(VMError::DecodeError);
                }
                let element_start = node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                let _ = argument_subtree_end(nodes, element_start)?;
                actions.extend(core::iter::repeat_n(
                    element_start,
                    usize::from(*item_count),
                ));
            }
            EntrypointValueTypeNodeV1::Leaf(kind) => {
                let atom = atoms.get(cursor).ok_or(VMError::DecodeError)?;
                cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                match (kind, atom) {
                    (EntrypointValueKindV1::Int, EntrypointValueAtomV1::Int(_))
                    | (EntrypointValueKindV1::Bool, EntrypointValueAtomV1::Bool(_)) => {}
                    (kind, EntrypointValueAtomV1::Pointer(envelope)) if kind.is_pointer() => {
                        validate_pointer_atom(policy, *kind, envelope)?;
                    }
                    _ => return Err(VMError::DecodeError),
                }
            }
        }
    }

    *node_index = end;
    *atom_index = cursor;
    Ok(())
}

fn validate_record_shape(
    schema: &EntrypointArgumentSchemaV1,
    schema_bytes: &[u8],
    record: &EntrypointArgumentRecordV1,
    policy: ivm_abi::SyscallPolicy,
) -> Result<Vec<EntrypointValueWordKindV1>, VMError> {
    if record.schema_hash != entrypoint_argument_schema_hash_v1(schema_bytes)
        || !schema.validate_atoms(&record.atoms)
    {
        return Err(VMError::DecodeError);
    }
    let mut atom_index = 0;
    for field in &schema.fields {
        let mut node_index = 0;
        validate_argument_atoms(
            policy,
            &field.ty.nodes,
            &record.atoms,
            &mut node_index,
            &mut atom_index,
        )?;
        if node_index != field.ty.nodes.len() {
            return Err(VMError::DecodeError);
        }
    }
    if atom_index != record.atoms.len() {
        return Err(VMError::DecodeError);
    }
    let word_kinds = schema
        .word_kinds_for_atoms(&record.atoms)
        .ok_or(VMError::DecodeError)?;
    Ok(word_kinds)
}

/// Validate canonical record bytes against an exact compiler-emitted schema.
///
/// The byte bound is checked before Norito decoding. Canonical re-encoding,
/// schema binding, active-only atom shape, pointer envelopes, and typed
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

fn plan_argument_atoms(
    nodes: &[EntrypointValueTypeNodeV1],
    atoms: &[EntrypointValueAtomV1],
    node_index: &mut usize,
    atom_index: &mut usize,
    decoded: &mut Vec<DecodedArgument>,
    roots: &mut Vec<usize>,
) -> Result<(), VMError> {
    enum Completion {
        Root,
        Sum {
            layout: SumLayoutV1,
            tag: u64,
            expected_words: usize,
        },
        ListItem,
    }

    struct PlanFrame {
        actions: Vec<usize>,
        roots: Vec<usize>,
        completion: Completion,
    }

    struct ListFrame {
        layout: ListLayoutV1,
        element_start: usize,
        element_words: usize,
        remaining_items: usize,
        elements: Vec<Vec<usize>>,
    }

    enum Frame {
        Plan(PlanFrame),
        List(ListFrame),
    }

    let start = *node_index;
    let end = argument_subtree_end(nodes, start)?;
    let mut frames = vec![Frame::Plan(PlanFrame {
        actions: vec![start],
        roots: Vec::new(),
        completion: Completion::Root,
    })];
    let mut cursor = *atom_index;
    let mut final_roots = None::<Vec<usize>>;

    while !frames.is_empty() {
        let plan_complete = matches!(
            frames.last(),
            Some(Frame::Plan(frame)) if frame.actions.is_empty()
        );
        if plan_complete {
            let Frame::Plan(frame) = frames.pop().expect("frame exists") else {
                unreachable!("completion predicate checked the frame variant");
            };
            match frame.completion {
                Completion::Root => {
                    if !frames.is_empty() {
                        return Err(VMError::DecodeError);
                    }
                    final_roots = Some(frame.roots);
                }
                Completion::Sum {
                    layout,
                    tag,
                    expected_words,
                } => {
                    if frame.roots.len() != expected_words {
                        return Err(VMError::DecodeError);
                    }
                    let index = decoded.len();
                    decoded.push(DecodedArgument::Sum {
                        layout,
                        tag,
                        active: frame.roots,
                    });
                    let Some(Frame::Plan(parent)) = frames.last_mut() else {
                        return Err(VMError::DecodeError);
                    };
                    parent.roots.push(index);
                }
                Completion::ListItem => {
                    let Some(Frame::List(_)) = frames.last() else {
                        return Err(VMError::DecodeError);
                    };
                    let Frame::List(mut list) = frames.pop().expect("list frame exists") else {
                        unreachable!("variant checked");
                    };
                    if frame.roots.len() != list.element_words {
                        return Err(VMError::DecodeError);
                    }
                    list.elements.push(frame.roots);
                    if list.remaining_items != 0 {
                        list.remaining_items = list
                            .remaining_items
                            .checked_sub(1)
                            .ok_or(VMError::DecodeError)?;
                        let element_start = list.element_start;
                        frames.push(Frame::List(list));
                        frames.push(Frame::Plan(PlanFrame {
                            actions: vec![element_start],
                            roots: Vec::new(),
                            completion: Completion::ListItem,
                        }));
                    } else {
                        let index = decoded.len();
                        decoded.push(DecodedArgument::List {
                            layout: list.layout,
                            elements: list.elements,
                        });
                        let Some(Frame::Plan(parent)) = frames.last_mut() else {
                            return Err(VMError::DecodeError);
                        };
                        parent.roots.push(index);
                    }
                }
            }
            continue;
        }

        let mut spawned = Vec::<Frame>::new();
        {
            let Some(Frame::Plan(frame)) = frames.last_mut() else {
                return Err(VMError::DecodeError);
            };
            let node_start = frame.actions.pop().ok_or(VMError::DecodeError)?;
            let node = nodes.get(node_start).ok_or(VMError::DecodeError)?;
            match node {
                EntrypointValueTypeNodeV1::Struct(node) => {
                    let starts = argument_child_starts(nodes, node_start, node.fields.len())?;
                    frame.actions.extend(starts.into_iter().rev());
                }
                EntrypointValueTypeNodeV1::Tuple(arity) => {
                    let starts = argument_child_starts(nodes, node_start, usize::from(*arity))?;
                    frame.actions.extend(starts.into_iter().rev());
                }
                EntrypointValueTypeNodeV1::Option => {
                    let EntrypointValueAtomV1::Tag(tag) =
                        atoms.get(cursor).ok_or(VMError::DecodeError)?
                    else {
                        return Err(VMError::DecodeError);
                    };
                    cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                    let child_start = node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                    let mut child_end = child_start;
                    let child_words = argument_node_word_count(nodes, &mut child_end)?;
                    let layout = SumLayoutV1::option(
                        u64::try_from(child_words).map_err(|_| VMError::DecodeError)?,
                    )
                    .map_err(|_| VMError::DecodeError)?;
                    if *tag {
                        spawned.push(Frame::Plan(PlanFrame {
                            actions: vec![child_start],
                            roots: Vec::with_capacity(child_words),
                            completion: Completion::Sum {
                                layout,
                                tag: 1,
                                expected_words: child_words,
                            },
                        }));
                    } else {
                        let index = decoded.len();
                        decoded.push(DecodedArgument::Sum {
                            layout,
                            tag: 0,
                            active: Vec::new(),
                        });
                        frame.roots.push(index);
                    }
                }
                EntrypointValueTypeNodeV1::Result => {
                    let EntrypointValueAtomV1::Tag(tag) =
                        atoms.get(cursor).ok_or(VMError::DecodeError)?
                    else {
                        return Err(VMError::DecodeError);
                    };
                    cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                    let ok_start = node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                    let mut ok_end = ok_start;
                    let ok_words = argument_node_word_count(nodes, &mut ok_end)?;
                    let mut err_end = ok_end;
                    let err_words = argument_node_word_count(nodes, &mut err_end)?;
                    let (active_start, active_words) = if *tag {
                        (ok_start, ok_words)
                    } else {
                        (ok_end, err_words)
                    };
                    let layout = SumLayoutV1::try_new(
                        u64::try_from(err_words).map_err(|_| VMError::DecodeError)?,
                        u64::try_from(ok_words).map_err(|_| VMError::DecodeError)?,
                    )
                    .map_err(|_| VMError::DecodeError)?;
                    spawned.push(Frame::Plan(PlanFrame {
                        actions: vec![active_start],
                        roots: Vec::with_capacity(active_words),
                        completion: Completion::Sum {
                            layout,
                            tag: u64::from(*tag),
                            expected_words: active_words,
                        },
                    }));
                }
                EntrypointValueTypeNodeV1::List(list) => {
                    let EntrypointValueAtomV1::List(item_count) =
                        atoms.get(cursor).ok_or(VMError::DecodeError)?
                    else {
                        return Err(VMError::DecodeError);
                    };
                    cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                    if *item_count > list.capacity {
                        return Err(VMError::DecodeError);
                    }
                    let element_start = node_start.checked_add(1).ok_or(VMError::DecodeError)?;
                    let mut element_end = element_start;
                    let element_words = argument_node_word_count(nodes, &mut element_end)?;
                    let layout = ListLayoutV1::try_new(
                        u64::from(list.capacity),
                        u64::try_from(element_words).map_err(|_| VMError::DecodeError)?,
                    )
                    .map_err(|_| VMError::DecodeError)?;
                    let item_count = usize::from(*item_count);
                    if item_count != 0 {
                        spawned.push(Frame::List(ListFrame {
                            layout,
                            element_start,
                            element_words,
                            remaining_items: item_count - 1,
                            elements: Vec::with_capacity(item_count),
                        }));
                        spawned.push(Frame::Plan(PlanFrame {
                            actions: vec![element_start],
                            roots: Vec::with_capacity(element_words),
                            completion: Completion::ListItem,
                        }));
                    } else {
                        let index = decoded.len();
                        decoded.push(DecodedArgument::List {
                            layout,
                            elements: Vec::new(),
                        });
                        frame.roots.push(index);
                    }
                }
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int) => {
                    let EntrypointValueAtomV1::Int(value) =
                        atoms.get(cursor).ok_or(VMError::DecodeError)?
                    else {
                        return Err(VMError::DecodeError);
                    };
                    cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                    let index = decoded.len();
                    decoded.push(DecodedArgument::Scalar(*value as u64));
                    frame.roots.push(index);
                }
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool) => {
                    let EntrypointValueAtomV1::Bool(value) =
                        atoms.get(cursor).ok_or(VMError::DecodeError)?
                    else {
                        return Err(VMError::DecodeError);
                    };
                    cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                    let index = decoded.len();
                    decoded.push(DecodedArgument::Scalar(u64::from(*value)));
                    frame.roots.push(index);
                }
                EntrypointValueTypeNodeV1::Leaf(_) => {
                    let EntrypointValueAtomV1::Pointer(envelope) =
                        atoms.get(cursor).ok_or(VMError::DecodeError)?
                    else {
                        return Err(VMError::DecodeError);
                    };
                    cursor = cursor.checked_add(1).ok_or(VMError::DecodeError)?;
                    let index = decoded.len();
                    decoded.push(DecodedArgument::Pointer(envelope.clone()));
                    frame.roots.push(index);
                }
            }
        }
        frames.extend(spawned);
    }

    let planned_roots = final_roots.ok_or(VMError::DecodeError)?;
    *node_index = end;
    *atom_index = cursor;
    roots.extend(planned_roots);
    Ok(())
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
    let mut roots = Vec::with_capacity(word_kinds.len());
    let mut atom_index = 0;
    for field in &schema.fields {
        let mut node_index = 0;
        plan_argument_atoms(
            &field.ty.nodes,
            &record.atoms,
            &mut node_index,
            &mut atom_index,
            &mut decoded,
            &mut roots,
        )?;
        if node_index != field.ty.nodes.len() {
            return Err(VMError::DecodeError);
        }
    }
    if atom_index != record.atoms.len() || roots.len() != word_kinds.len() {
        return Err(VMError::DecodeError);
    }

    let (pointer_envelope_bytes, pointer_allocation_bytes) = decoded
        .iter()
        .filter_map(|value| match value {
            DecodedArgument::Pointer(envelope) => Some((
                u64::try_from(envelope.len()).unwrap_or(u64::MAX),
                aligned_allocation_bytes(envelope.len()),
            )),
            _ => None,
        })
        .fold((0_u64, 0_u64), |totals, bytes| {
            (
                totals.0.saturating_add(bytes.0),
                totals.1.saturating_add(bytes.1),
            )
        });
    if pointer_envelope_bytes > u64::try_from(record_bytes).unwrap_or(u64::MAX) {
        return Err(VMError::DecodeError);
    }
    let raw_heap_bytes = decoded
        .iter()
        .filter_map(|value| match value {
            DecodedArgument::Sum { layout, .. } => {
                Some(layout.allocation_bytes().unwrap_or(u64::MAX))
            }
            DecodedArgument::List { layout, .. } => {
                Some(layout.allocation_bytes().unwrap_or(u64::MAX))
            }
            DecodedArgument::Scalar(_) | DecodedArgument::Pointer(_) => None,
        })
        .fold(0_u64, u64::saturating_add);
    // No successful invocation can materialize more than the VM's owned HEAP.
    // Capping the charged raw-allocation component keeps the predecode schema
    // bound valid even for a value which will later fail the allocation
    // preflight; pointer copies and the output table remain charged exactly.
    let materialized_bytes = aligned_allocation_bytes(decoded_table_envelope_len(roots.len()))
        .saturating_add(pointer_allocation_bytes)
        .saturating_add(cap_raw_heap_bytes(raw_heap_bytes));
    Ok(ArgumentDecodePlan {
        decoded,
        roots,
        record_bytes,
        schema_bytes: schema_bytes.len(),
        materialized_bytes,
    })
}

/// Validate and prepare a canonical record only after its deterministic work
/// fits the invocation's gas allowance.
///
/// The compiler-owned schema is validated first. Its flat preorder tape then
/// yields a bounded, allocation-free maximum for ABI words and aggregate HEAP
/// storage. Together with signed wire lengths and the record-bounded aligned
/// pointer-copy allowance, that quote is checked before the first untrusted
/// record decode.
///
/// # Errors
///
/// Returns [`VMError::OutOfGas`] when the deterministic preparation bound is
/// unaffordable, or a decode/ABI/memory error when the bounded record is not a
/// canonical executable V1 argument record.
pub fn prepare_argument_record_with_gas_limit(
    schema: &EntrypointArgumentSchemaV1,
    canonical_record: Arc<[u8]>,
    gas_limit: u64,
) -> Result<PreparedArgumentRecord, VMError> {
    if canonical_record.len() > MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES {
        return Err(VMError::DecodeError);
    }
    let schema_bound = schema_materialization_bound(schema)?;
    let schema_bytes_len = canonical_norito_frame_len(schema);
    if schema_bytes_len > MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES {
        return Err(VMError::DecodeError);
    }
    let gas_bound = argument_record_gas_for_schema_bound(
        canonical_record.len(),
        schema_bytes_len,
        schema_bound,
    );
    if gas_bound > gas_limit {
        return Err(VMError::OutOfGas);
    }
    let schema_bytes: Arc<[u8]> = Arc::from(to_bytes(schema).map_err(|_| VMError::DecodeError)?);
    debug_assert_eq!(schema_bytes.len(), schema_bytes_len);
    let record = decode_record(&canonical_record)?;
    let decode_plan = build_decode_plan(
        schema,
        &schema_bytes,
        record,
        ivm_abi::SyscallPolicy::AbiV1,
        canonical_record.len(),
    )?;
    if decode_plan.gas() > gas_bound {
        return Err(VMError::DecodeError);
    }
    let binding = argument_record_binding(&canonical_record);
    // Compiler-generated wrappers load the schema from validated program data,
    // so only the host-issued binding and decoded outputs consume VM arenas.
    let mut fresh_allocation_lengths = Vec::with_capacity(decode_plan.decoded.len() + 2);
    fresh_allocation_lengths.push(TLV_ENVELOPE_BYTES + binding.len());
    fresh_allocation_lengths.extend(decode_plan.allocation_lengths());
    let raw_heap_bytes = decode_plan.raw_heap_bytes();
    IVM::preflight_fresh_host_tlv_allocations_with_reserved_heap(
        &fresh_allocation_lengths,
        raw_heap_bytes,
    )?;
    Ok(PreparedArgumentRecord {
        inner: Arc::new(PreparedArgumentRecordInner {
            canonical_record,
            canonical_schema: schema_bytes,
            binding,
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

/// Compute a conservative gas reserve for [`decode_argument_record`] without
/// decoding either payload, allocating, or changing VM state.
pub(crate) fn decode_argument_record_gas_quote(vm: &IVM) -> Result<u64, VMError> {
    let (record_bytes, schema_bytes) = quote_argument_envelope_lengths(vm)?;
    Ok(argument_record_runtime_gas_upper_bound(
        record_bytes,
        schema_bytes,
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
    let raw_heap_bytes = plan.raw_heap_bytes();
    vm.preflight_host_tlv_allocations_with_reserved_heap(
        &plan.allocation_lengths(),
        raw_heap_bytes,
    )?;

    let mut materialized = Vec::<u64>::with_capacity(plan.decoded.len());
    for (index, value) in plan.decoded.iter().enumerate() {
        let word = match value {
            DecodedArgument::Scalar(value) => Ok(*value),
            DecodedArgument::Pointer(envelope) => vm.alloc_host_tlv(envelope),
            DecodedArgument::Sum {
                layout,
                tag,
                active,
            } => {
                let mut payload = Vec::with_capacity(active.len());
                for child in active {
                    if *child >= index {
                        return Err(VMError::DecodeError);
                    }
                    payload.push(*materialized.get(*child).ok_or(VMError::DecodeError)?);
                }
                crate::sum::allocate_words(vm, *layout, *tag, &payload)
            }
            DecodedArgument::List { layout, elements } => {
                let width =
                    usize::try_from(layout.element_words()).map_err(|_| VMError::DecodeError)?;
                let mut words = Vec::with_capacity(elements.len());
                for element in elements {
                    let mut item = Vec::with_capacity(width);
                    for child in element {
                        if *child >= index {
                            return Err(VMError::DecodeError);
                        }
                        item.push(*materialized.get(*child).ok_or(VMError::DecodeError)?);
                    }
                    if item.len() != width {
                        return Err(VMError::DecodeError);
                    }
                    words.push(item);
                }
                crate::list::allocate_words(vm, *layout, &words)
            }
        }?;
        materialized.push(word);
    }

    let mut words = Vec::with_capacity(plan.roots.len());
    for root in &plan.roots {
        words.push(*materialized.get(*root).ok_or(VMError::DecodeError)?);
    }

    let mut table = Vec::with_capacity(1 + words.len() * core::mem::size_of::<u64>());
    table.push(0);
    for word in words {
        table.extend_from_slice(&word.to_le_bytes());
    }
    let table_pointer = vm.alloc_host_tlv(&encode_tlv(PointerType::Blob, &table)?)?;
    vm.set_register(10, table_pointer);
    Ok(gas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ivm_abi::entrypoint::{
        EntrypointArgumentFieldV1, EntrypointListTypeNodeV1, EntrypointStructTypeNodeV1,
        EntrypointValueKindV1, EntrypointValueTypeNodeV1, EntrypointValueTypeV1,
        MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH,
    };

    fn argument_type(kind: EntrypointValueKindV1) -> EntrypointValueTypeV1 {
        EntrypointValueTypeV1 {
            nodes: vec![EntrypointValueTypeNodeV1::Leaf(kind)],
        }
    }

    fn list_type(capacity: u8, element: EntrypointValueTypeV1) -> EntrypointValueTypeV1 {
        let mut nodes = Vec::with_capacity(element.nodes.len().saturating_add(1));
        nodes.push(EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
            capacity,
        }));
        nodes.extend(element.nodes);
        EntrypointValueTypeV1 { nodes }
    }

    fn nested_list_type(levels: usize) -> EntrypointValueTypeV1 {
        let mut nodes = Vec::with_capacity(levels.saturating_add(1));
        for _ in 0..levels {
            nodes.push(EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
                capacity: 1,
            }));
        }
        nodes.push(EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int));
        EntrypointValueTypeV1 { nodes }
    }

    fn prepared_gas_bound(schema: &EntrypointArgumentSchemaV1, record_bytes: usize) -> u64 {
        let schema_bytes = to_bytes(schema).expect("encode schema for gas bound");
        let bound = schema_materialization_bound(schema).expect("valid schema bound");
        argument_record_gas_for_schema_bound(record_bytes, schema_bytes.len(), bound)
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
                    ty: argument_type(EntrypointValueKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "label".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Name),
                },
                EntrypointArgumentFieldV1 {
                    name: "bytes".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Blob),
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
        let table = vm.validate_tlv(vm.register(10)).expect("result table TLV");
        assert_eq!(table.type_id, PointerType::Blob);
        assert_eq!(table.payload.len(), 1 + 3 * core::mem::size_of::<u64>());
        assert_eq!(table.payload[0], 0, "alignment prefix must be canonical");
        let count = u64::from_le_bytes(table.payload[1..9].try_into().expect("count word"));
        assert_eq!(count, 7);
        for bytes in [9..17, 17..25] {
            let pointer =
                u64::from_le_bytes(table.payload[bytes].try_into().expect("pointer word"));
            vm.validate_tlv(pointer).expect("typed output TLV");
        }
    }

    #[test]
    fn prepared_record_materializes_without_a_second_record_decode() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "count".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "label".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Name),
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
            prepare_argument_record_with_gas_limit(&schema, Arc::clone(&canonical), u64::MAX)
                .expect("prepare arguments");
        let shared = prepared.clone();
        assert!(core::ptr::eq(
            prepared.canonical_bytes().as_ptr(),
            shared.canonical_bytes().as_ptr(),
        ));
        let undomained_hash: [u8; Hash::LENGTH] = Hash::new(canonical.as_ref()).into();
        assert_ne!(
            *prepared.binding_bytes(),
            undomained_hash,
            "the guest capability must not be the bare record digest"
        );

        let mut vm = IVM::new(u64::MAX);
        let record_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.binding_bytes());
        let schema_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.schema_bytes());
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        prepared
            .precharge_vm(&mut vm)
            .expect("precharge prepared arguments");
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
                ty: argument_type(EntrypointValueKindV1::Int),
            }],
        };
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(&schema, &Json::from(norito::json!({"count": 7})))
                .expect("encode argument record"),
        );
        let prepared =
            prepare_argument_record_with_gas_limit(&schema, Arc::clone(&canonical), u64::MAX)
                .expect("prepare arguments");
        let mut vm = IVM::new(u64::MAX);
        let issued_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.binding_bytes());
        let substituted_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.binding_bytes());
        let schema_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.schema_bytes());
        vm.set_register(10, substituted_ptr);
        vm.set_register(11, schema_ptr);
        prepared
            .precharge_vm(&mut vm)
            .expect("precharge prepared arguments");
        assert!(matches!(
            prepared.decode_gas_quote(&vm, issued_ptr),
            Err(VMError::DecodeError)
        ));

        let other_schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "different".to_owned(),
                ty: argument_type(EntrypointValueKindV1::Int),
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
    fn prepared_quote_defers_same_length_schema_authentication_until_after_precharge() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "count".to_owned(),
                ty: argument_type(EntrypointValueKindV1::Int),
            }],
        };
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(&schema, &Json::from(norito::json!({"count": 7})))
                .expect("encode argument record"),
        );
        let prepared = prepare_argument_record_with_gas_limit(&schema, canonical, u64::MAX)
            .expect("prepare arguments");
        let mut vm = IVM::new(u64::MAX);
        let issued_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.binding_bytes());
        let mut substituted_schema = prepared.schema_bytes().to_vec();
        let last = substituted_schema
            .last_mut()
            .expect("canonical schema is non-empty");
        *last ^= 1;
        let substituted_schema_ptr = alloc(&mut vm, PointerType::NoritoBytes, &substituted_schema);
        vm.set_register(10, issued_ptr);
        vm.set_register(11, substituted_schema_ptr);
        prepared
            .precharge_vm(&mut vm)
            .expect("precharge prepared arguments");

        assert_eq!(
            prepared.decode_gas_quote(&vm, issued_ptr),
            Ok(0),
            "prepare checks only bounded envelope shape"
        );
        assert_eq!(
            prepared.install_into_vm(&mut vm, issued_ptr),
            Err(VMError::DecodeError),
            "the post-debit path authenticates the exact schema bytes"
        );
    }

    #[test]
    fn unaffordable_preparation_rejects_before_canonical_record_decode() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "payload".to_owned(),
                ty: argument_type(EntrypointValueKindV1::String),
            }],
        };
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(
                &schema,
                &Json::from(norito::json!({ "payload": "meter me first" })),
            )
            .expect("encode argument record"),
        );
        let quote = prepared_gas_bound(&schema, canonical.len());

        RECORD_DECODE_COUNT.with(|count| count.set(0));
        assert!(matches!(
            prepare_argument_record_with_gas_limit(
                &schema,
                Arc::clone(&canonical),
                quote.saturating_sub(1),
            ),
            Err(VMError::OutOfGas)
        ));
        RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 0));

        prepare_argument_record_with_gas_limit(&schema, canonical, quote)
            .expect("the exact conservative preparation quote is affordable");
        RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 1));
    }

    #[test]
    fn allocation_free_schema_length_matches_canonical_norito_frame() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "count".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "payload".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Blob),
                },
            ],
        };
        let canonical = to_bytes(&schema).expect("encode canonical schema");
        assert_eq!(canonical_norito_frame_len(&schema), canonical.len());
    }

    #[test]
    fn large_prepared_argument_spills_to_heap_without_input_arena_trap() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "payload".to_owned(),
                ty: argument_type(EntrypointValueKindV1::String),
            }],
        };
        let large = "x".repeat((crate::memory::Memory::INPUT_SIZE as usize) * 2);
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(
                &schema,
                &Json::from(norito::json!({ "payload": large })),
            )
            .expect("encode large argument record"),
        );
        let prepared = prepare_argument_record_with_gas_limit(&schema, canonical, u64::MAX)
            .expect("prepare large record");
        let mut vm = IVM::new(u64::MAX);
        let record_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.binding_bytes());
        let schema_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.schema_bytes());
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        prepared
            .precharge_vm(&mut vm)
            .expect("precharge large prepared value");

        prepared
            .install_into_vm(&mut vm, record_ptr)
            .expect("large prepared value must use bounded HEAP spill");
        assert!(
            (crate::memory::Memory::INPUT_START
                ..crate::memory::Memory::INPUT_START + crate::memory::Memory::INPUT_SIZE)
                .contains(&vm.register(10)),
            "the word table should still prefer available INPUT capacity"
        );
        let words = decoded_words(&vm);
        assert_eq!(words.len(), 1);
        assert!(
            (crate::memory::Memory::HEAP_START..crate::memory::Memory::INPUT_START)
                .contains(&words[0]),
            "the typed value larger than INPUT must be materialized in owned HEAP"
        );
    }

    #[test]
    fn exact_wire_cap_record_is_admitted_and_materialized() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "payload".to_owned(),
                ty: argument_type(EntrypointValueKindV1::Blob),
            }],
        };
        let schema_bytes = to_bytes(&schema).expect("encode schema");
        let mut payload_len = MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES.saturating_sub(256);
        let mut canonical = Vec::new();
        for _ in 0..8 {
            let envelope = encode_tlv(PointerType::Blob, &vec![0x5A; payload_len])
                .expect("encode exact-bound blob envelope");
            canonical = to_bytes(&EntrypointArgumentRecordV1 {
                schema_hash: entrypoint_argument_schema_hash_v1(&schema_bytes),
                atoms: vec![EntrypointValueAtomV1::Pointer(envelope)],
            })
            .expect("encode exact-bound record");
            match canonical.len().cmp(&MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES) {
                core::cmp::Ordering::Equal => break,
                core::cmp::Ordering::Less => {
                    payload_len = payload_len
                        .saturating_add(MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES - canonical.len());
                }
                core::cmp::Ordering::Greater => {
                    payload_len = payload_len
                        .saturating_sub(canonical.len() - MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES);
                }
            }
        }
        assert_eq!(
            canonical.len(),
            MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES,
            "the fixture must exercise the inclusive signed wire boundary"
        );

        let gas_bound = prepared_gas_bound(&schema, canonical.len());
        let prepared =
            prepare_argument_record_with_gas_limit(&schema, Arc::from(canonical), gas_bound)
                .expect("the exact V1 wire cap must be executable at its schema-derived gas bound");
        let mut vm = IVM::new(u64::MAX);
        let binding_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.binding_bytes());
        let schema_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.schema_bytes());
        vm.set_register(10, binding_ptr);
        vm.set_register(11, schema_ptr);
        prepared
            .precharge_vm(&mut vm)
            .expect("precharge exact-cap prepared value");
        prepared
            .install_into_vm(&mut vm, binding_ptr)
            .expect("exact-cap pointer atom and word table fit the V1 VM resource envelope");
        let words = decoded_words(&vm);
        assert_eq!(words.len(), 1);
        assert!(
            (crate::memory::Memory::HEAP_START..crate::memory::Memory::INPUT_START)
                .contains(&words[0])
        );
    }

    #[test]
    fn materialization_preflight_prevents_partial_allocation_on_oom() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "small".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::String),
                },
                EntrypointArgumentFieldV1 {
                    name: "large".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::String),
                },
            ],
        };
        let large = "x".repeat((crate::memory::Memory::INPUT_SIZE as usize) * 2);
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(
                &schema,
                &Json::from(norito::json!({ "small": "fits-input", "large": large })),
            )
            .expect("encode large argument record"),
        );
        let prepared = prepare_argument_record_with_gas_limit(&schema, canonical, u64::MAX)
            .expect("prepare large record");
        let mut vm = IVM::new(u64::MAX);
        let record_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.binding_bytes());
        let schema_ptr = alloc(&mut vm, PointerType::NoritoBytes, prepared.schema_bytes());
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        prepared
            .precharge_vm(&mut vm)
            .expect("precharge prepared value");
        vm.memory
            .set_heap_limit(crate::memory::Memory::INPUT_SIZE)
            .expect("constrain test heap");
        let mut allocation_control = vm.clone();

        assert_eq!(
            prepared.install_into_vm(&mut vm, record_ptr),
            Err(VMError::OutOfMemory)
        );
        assert_eq!(
            vm.memory.heap_allocated_len(),
            0,
            "capacity failure must occur before the first guest-visible allocation"
        );
        assert_eq!(vm.register(10), record_ptr);
        assert_eq!(vm.register(11), schema_ptr);
        let sentinel = encode_tlv(PointerType::Blob, b"after-preflight").expect("encode sentinel");
        assert_eq!(
            vm.alloc_input_tlv(&sentinel)
                .expect("allocate after failed preflight"),
            allocation_control
                .alloc_input_tlv(&sentinel)
                .expect("allocate in untouched control"),
            "capacity rejection must not consume INPUT for an earlier small pointer"
        );
    }

    #[test]
    fn materialization_preflight_combines_tlv_spill_and_list_heap_capacity() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "values".to_owned(),
                ty: list_type(8, argument_type(EntrypointValueKindV1::String)),
            }],
        };
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(
                &schema,
                &Json::from(norito::json!({ "values": ["heap-spill"] })),
            )
            .expect("encode list argument record"),
        );
        let prepared = prepare_argument_record_with_gas_limit(&schema, canonical, u64::MAX)
            .expect("prepare list argument record");

        let mut vm = IVM::new(u64::MAX);
        vm.alloc_input_tlv(&vec![0_u8; crate::memory::Memory::INPUT_SIZE as usize])
            .expect("fill INPUT exactly");
        let binding_envelope =
            encode_tlv(PointerType::NoritoBytes, prepared.binding_bytes()).expect("binding TLV");
        let record_ptr = vm
            .alloc_host_tlv(&binding_envelope)
            .expect("spill binding to HEAP");
        let schema_envelope =
            encode_tlv(PointerType::NoritoBytes, prepared.schema_bytes()).expect("schema TLV");
        let schema_ptr = vm
            .alloc_host_tlv(&schema_envelope)
            .expect("spill schema to HEAP");
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);

        let plan = &prepared.inner.decode_plan;
        let raw_heap_bytes = plan.raw_heap_bytes();
        let spilled_tlv_bytes = plan
            .allocation_lengths()
            .into_iter()
            .map(|length| {
                u64::try_from(length)
                    .expect("bounded TLV length")
                    .checked_add(7)
                    .expect("bounded aligned TLV length")
                    & !7
            })
            .sum::<u64>();
        assert!(raw_heap_bytes > 0 && spilled_tlv_bytes > 0);
        let baseline = vm.memory.heap_allocated_len();
        let exact_combined_limit = baseline
            .checked_add(raw_heap_bytes)
            .and_then(|limit| limit.checked_add(spilled_tlv_bytes))
            .expect("bounded combined capacity");
        let mut exact_vm = vm.clone();
        exact_vm
            .memory
            .set_heap_limit(exact_combined_limit)
            .expect("set exact combined HEAP capacity");
        prepared
            .precharge_vm(&mut exact_vm)
            .expect("precharge exact-capacity VM after cloning");
        prepared
            .install_into_vm(&mut exact_vm, record_ptr)
            .expect("the exact combined HEAP limit must be inclusive");
        assert_eq!(exact_vm.memory.heap_allocated_len(), exact_combined_limit);

        vm.memory
            .set_heap_limit(exact_combined_limit - 1)
            .expect("constrain combined HEAP capacity by one byte");
        prepared
            .precharge_vm(&mut vm)
            .expect("precharge constrained VM");
        let mut allocation_control = vm.clone();

        assert_eq!(
            prepared.install_into_vm(&mut vm, record_ptr),
            Err(VMError::OutOfMemory)
        );
        assert_eq!(vm.memory.heap_allocated_len(), baseline);
        assert_eq!((vm.register(10), vm.register(11)), (record_ptr, schema_ptr));
        let sentinel =
            encode_tlv(PointerType::Blob, b"after-combined-preflight").expect("encode sentinel");
        assert_eq!(
            vm.alloc_host_tlv(&sentinel)
                .expect("allocate after combined preflight failure"),
            allocation_control
                .alloc_host_tlv(&sentinel)
                .expect("allocate in untouched control"),
            "combined capacity rejection must precede every output allocation"
        );
    }

    #[test]
    fn prepared_outputs_spill_pointer_and_word_table_to_owned_heap() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "payload".to_owned(),
                ty: argument_type(EntrypointValueKindV1::String),
            }],
        };
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(
                &schema,
                &Json::from(norito::json!({ "payload": "heap-owned" })),
            )
            .expect("encode argument record"),
        );
        let prepared = prepare_argument_record_with_gas_limit(&schema, canonical, u64::MAX)
            .expect("prepare arguments");
        let mut vm = IVM::new(u64::MAX);
        vm.alloc_input_tlv(&vec![0_u8; crate::memory::Memory::INPUT_SIZE as usize])
            .expect("fill INPUT exactly");
        let binding_envelope =
            encode_tlv(PointerType::NoritoBytes, prepared.binding_bytes()).expect("binding TLV");
        let record_ptr = vm
            .alloc_host_tlv(&binding_envelope)
            .expect("spill binding to HEAP");
        let schema_envelope =
            encode_tlv(PointerType::NoritoBytes, prepared.schema_bytes()).expect("schema TLV");
        let schema_ptr = vm
            .alloc_host_tlv(&schema_envelope)
            .expect("spill schema to HEAP");
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        prepared
            .precharge_vm(&mut vm)
            .expect("precharge prepared arguments");

        prepared
            .install_into_vm(&mut vm, record_ptr)
            .expect("spill the complete result sequence to HEAP");
        assert!(
            (crate::memory::Memory::HEAP_START..crate::memory::Memory::INPUT_START)
                .contains(&vm.register(10)),
            "the decoded word table must be in the owned HEAP prefix"
        );
        let words = decoded_words(&vm);
        assert_eq!(words.len(), 1);
        assert!(
            (crate::memory::Memory::HEAP_START..crate::memory::Memory::INPUT_START)
                .contains(&words[0]),
            "the decoded pointer TLV must be in the owned HEAP prefix"
        );
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
                    ty: argument_type(EntrypointValueKindV1::Bool),
                },
                EntrypointArgumentFieldV1 {
                    name: "memo".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::String),
                },
                EntrypointArgumentFieldV1 {
                    name: "wide".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::U128),
                },
                EntrypointArgumentFieldV1 {
                    name: "account".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::AccountId),
                },
                EntrypointArgumentFieldV1 {
                    name: "definition".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::AssetDefinitionId),
                },
                EntrypointArgumentFieldV1 {
                    name: "asset".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::AssetId),
                },
                EntrypointArgumentFieldV1 {
                    name: "domain".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::DomainId),
                },
                EntrypointArgumentFieldV1 {
                    name: "nft".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::NftId),
                },
                EntrypointArgumentFieldV1 {
                    name: "dataspace".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::DataSpaceId),
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
                ty: EntrypointValueTypeV1 {
                    nodes: vec![
                        EntrypointValueTypeNodeV1::Struct(
                            ivm_abi::entrypoint::EntrypointStructTypeNodeV1 {
                                name: "Request".into(),
                                fields: vec!["pair".into(), "memo".into(), "outcome".into()],
                            },
                        ),
                        EntrypointValueTypeNodeV1::Tuple(2),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
                        EntrypointValueTypeNodeV1::Option,
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
                        EntrypointValueTypeNodeV1::Result,
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Name),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
                    ],
                },
            }],
        };
        assert_eq!(schema.word_count(), Some(4));
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
        assert_eq!(words.len(), 4);
        assert_eq!(words[0], 7);
        assert_eq!(words[1], 1);
        let (some, memo) = crate::sum::read_words(
            &vm,
            words[2],
            SumLayoutV1::option(1).expect("Option layout"),
        )
        .expect("read Option");
        assert!(some);
        assert_eq!(memo.len(), 1);
        let memo = vm.memory.validate_tlv(memo[0]).expect("memo string TLV");
        assert_eq!(memo.type_id, PointerType::Blob);
        assert_eq!(memo.payload, "言霊".as_bytes());
        let (ok, outcome) = crate::sum::read_words(
            &vm,
            words[3],
            SumLayoutV1::try_new(1, 1).expect("Result layout"),
        )
        .expect("read Result");
        assert!(!ok);
        assert_eq!(outcome, vec![1]);
    }

    #[test]
    fn nested_amount_lists_materialize_as_one_schema_bound_sequence() {
        let amount = argument_type(EntrypointValueKindV1::Amount);
        let inner = list_type(2, amount);
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "amounts".to_owned(),
                ty: list_type(2, inner),
            }],
        };
        let payload = Json::from(norito::json!({
            "amounts": [["1.25"], ["2"]],
        }));
        let mut vm = install_record(&schema, &payload);
        decode_argument_record(&mut vm).expect("decode nested amount list");
        let words = decoded_words(&vm);
        assert_eq!(words.len(), 1, "a bounded list is one VM handle");
        let outer_layout = ListLayoutV1::try_new(2, 1).expect("outer layout");
        let outer = crate::list::read_words(&vm, words[0], outer_layout).expect("read outer list");
        assert_eq!(outer.len(), 2);
        for item in outer {
            let inner_layout = ListLayoutV1::try_new(2, 1).expect("inner layout");
            let inner =
                crate::list::read_words(&vm, item[0], inner_layout).expect("read inner list");
            assert_eq!(inner.len(), 1, "inner list must contain one element");
            assert_eq!(
                inner[0].len(),
                1,
                "inner list element must contain one Amount pointer word"
            );
            let amount = vm.validate_tlv(inner[0][0]).expect("valid Amount TLV");
            assert_eq!(amount.type_id, PointerType::Amount);
            let numeric: Numeric = decode_from_bytes(amount.payload).expect("decode Amount");
            numeric.validate_amount().expect("canonical Amount");
        }

        let overflow = Json::from(norito::json!({
            "amounts": [[], [], []],
        }));
        assert_eq!(
            argument_record_from_json(&schema, &overflow),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn maximum_flat_list_depth_decodes_validates_and_materializes_on_the_test_thread() {
        let levels = MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1;
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: nested_list_type(levels),
            }],
        };
        assert!(schema.validate());

        let mut value = njson::Value::from(7_i64);
        for _ in 0..levels {
            value = njson::Value::Array(vec![value]);
        }
        let payload = Json::from(norito::json!({ "value": value }));
        let mut vm = install_record(&schema, &payload);
        decode_argument_record(&mut vm).expect("materialize the exact V1 nesting boundary");

        let mut word = decoded_words(&vm)[0];
        let layout = ListLayoutV1::try_new(1, 1).expect("unit-width list layout");
        for _ in 0..levels {
            let items =
                crate::list::read_words(&vm, word, layout).expect("read one nested list level");
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].len(), 1);
            word = items[0][0];
        }
        assert_eq!(word as i64, 7);

        let over_limit = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: nested_list_type(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH),
            }],
        };
        assert!(!over_limit.validate());
        assert_eq!(
            argument_record_from_json(&over_limit, &payload),
            Err(VMError::DecodeError),
        );
    }

    #[test]
    fn flat_list_element_subtree_controls_item_width_and_rejects_trailing_atoms() {
        let tuple = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Tuple(2),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
            ],
        };
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "pairs".to_owned(),
                ty: list_type(2, tuple),
            }],
        };
        let payload = Json::from(norito::json!({
            "pairs": [[7, true], [9, false]],
        }));
        let record = argument_record_from_json(&schema, &payload).expect("encode flat list tape");
        assert_eq!(
            record.atoms,
            vec![
                EntrypointValueAtomV1::List(2),
                EntrypointValueAtomV1::Int(7),
                EntrypointValueAtomV1::Bool(true),
                EntrypointValueAtomV1::Int(9),
                EntrypointValueAtomV1::Bool(false),
            ],
            "list items must live inline in the record's single preorder atom tape",
        );
        let mut vm = install_record(&schema, &payload);
        decode_argument_record(&mut vm).expect("decode flat list element subtree");
        let table = decoded_words(&vm);
        assert_eq!(table.len(), 1, "the list itself is one ABI word");
        assert_eq!(
            crate::list::read_words(
                &vm,
                table[0],
                ListLayoutV1::try_new(2, 2).expect("pair-list layout"),
            )
            .expect("read contiguous pair list"),
            vec![vec![7, 1], vec![9, 0]],
        );

        let schema_bytes = to_bytes(&schema).expect("encode pair-list schema");
        let schema_hash = entrypoint_argument_schema_hash_v1(&schema_bytes);
        for (label, atoms) in [
            (
                "trailing atom",
                vec![
                    EntrypointValueAtomV1::List(1),
                    EntrypointValueAtomV1::Int(7),
                    EntrypointValueAtomV1::Bool(true),
                    EntrypointValueAtomV1::Int(99),
                ],
            ),
            (
                "missing atom",
                vec![
                    EntrypointValueAtomV1::List(1),
                    EntrypointValueAtomV1::Int(7),
                ],
            ),
            (
                "wrong atom kind",
                vec![
                    EntrypointValueAtomV1::List(1),
                    EntrypointValueAtomV1::Bool(true),
                    EntrypointValueAtomV1::Bool(false),
                ],
            ),
            (
                "capacity overflow",
                vec![
                    EntrypointValueAtomV1::List(3),
                    EntrypointValueAtomV1::Int(1),
                    EntrypointValueAtomV1::Bool(true),
                    EntrypointValueAtomV1::Int(2),
                    EntrypointValueAtomV1::Bool(true),
                    EntrypointValueAtomV1::Int(3),
                    EntrypointValueAtomV1::Bool(true),
                ],
            ),
            (
                "item count exceeds available elements",
                vec![
                    EntrypointValueAtomV1::List(2),
                    EntrypointValueAtomV1::Int(1),
                    EntrypointValueAtomV1::Bool(true),
                ],
            ),
        ] {
            let record = EntrypointArgumentRecordV1 { schema_hash, atoms };
            let encoded = to_bytes(&record).expect("encode adversarial pair-list record");
            assert_eq!(
                validate_argument_record(&schema, &encoded),
                Err(VMError::DecodeError),
                "{label} must fail closed",
            );
        }
    }

    #[test]
    fn recursive_legacy_list_record_encoding_is_rejected() {
        #[derive(norito::Encode)]
        enum LegacyAtom {
            Tag(bool),
            Int(i64),
            Bool(bool),
            Pointer(Vec<u8>),
            List(Vec<Vec<LegacyAtom>>),
        }

        #[derive(norito::Encode)]
        struct LegacyRecord {
            schema_hash: [u8; 32],
            atoms: Vec<LegacyAtom>,
        }

        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "values".to_owned(),
                ty: list_type(1, argument_type(EntrypointValueKindV1::Int)),
            }],
        };
        let schema_bytes = to_bytes(&schema).expect("encode list schema");
        let legacy = LegacyRecord {
            schema_hash: entrypoint_argument_schema_hash_v1(&schema_bytes),
            atoms: vec![LegacyAtom::List(vec![vec![LegacyAtom::Int(7)]])],
        };
        let encoded = to_bytes(&legacy).expect("encode retired recursive list shape");

        assert_eq!(
            validate_argument_record(&schema, &encoded),
            Err(VMError::DecodeError),
            "the first release accepts only the flat list-count tape",
        );

        // Keep every retired discriminant represented so this test continues
        // to encode the exact former enum ordering instead of a lookalike.
        drop((
            LegacyAtom::Tag(false),
            LegacyAtom::Bool(false),
            LegacyAtom::Pointer(Vec::new()),
        ));
    }

    #[test]
    fn empty_flat_list_advances_past_its_element_subtree_before_a_sibling() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "request".to_owned(),
                ty: EntrypointValueTypeV1 {
                    nodes: vec![
                        EntrypointValueTypeNodeV1::Struct(
                            ivm_abi::entrypoint::EntrypointStructTypeNodeV1 {
                                name: "Request".to_owned(),
                                fields: vec!["pairs".to_owned(), "nonce".to_owned()],
                            },
                        ),
                        EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 2 }),
                        EntrypointValueTypeNodeV1::Tuple(2),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                    ],
                },
            }],
        };
        let mut vm = install_record(
            &schema,
            &Json::from(norito::json!({
                "request": { "pairs": [], "nonce": 41 },
            })),
        );
        decode_argument_record(&mut vm).expect("decode empty list before product sibling");
        let words = decoded_words(&vm);
        assert_eq!(words.len(), 2);
        assert!(
            crate::list::read_words(
                &vm,
                words[0],
                ListLayoutV1::try_new(2, 2).expect("empty pair-list layout"),
            )
            .expect("read empty list")
            .is_empty(),
        );
        assert_eq!(words[1], 41);
    }

    #[test]
    fn list_of_nested_option_results_materializes_active_only_sum_handles() {
        let element = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Result,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Amount),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
            ],
        };
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "values".to_owned(),
                ty: list_type(3, element),
            }],
        };
        let payload = Json::from(norito::json!({
            "values": [
                { "some": { "ok": "1.25" } },
                { "some": { "err": true } },
                { "none": true },
            ],
        }));
        let mut vm = install_record(&schema, &payload);
        decode_argument_record(&mut vm).expect("decode nested sums");
        let table = decoded_words(&vm);
        assert_eq!(table.len(), 1);
        let list = crate::list::read_words(
            &vm,
            table[0],
            ListLayoutV1::try_new(3, 1).expect("list layout"),
        )
        .expect("read list");
        assert_eq!(list.len(), 3);

        let option_layout = SumLayoutV1::option(1).expect("Option layout");
        let result_layout = SumLayoutV1::try_new(1, 1).expect("Result layout");
        let (some, first) =
            crate::sum::read_words(&vm, list[0][0], option_layout).expect("read first Option");
        assert!(some);
        let (ok, amount) =
            crate::sum::read_words(&vm, first[0], result_layout).expect("read first Result");
        assert!(ok);
        let amount = vm.validate_tlv(amount[0]).expect("Amount TLV");
        assert_eq!(amount.type_id, PointerType::Amount);
        let amount: Numeric = decode_from_bytes(amount.payload).expect("decode Amount");
        assert_eq!(amount, Numeric::new(125, 2));

        let (some, second) =
            crate::sum::read_words(&vm, list[1][0], option_layout).expect("read second Option");
        assert!(some);
        let (ok, error) =
            crate::sum::read_words(&vm, second[0], result_layout).expect("read second Result");
        assert!(!ok);
        assert_eq!(error, vec![1]);

        let (some, none_payload) =
            crate::sum::read_words(&vm, list[2][0], option_layout).expect("read Option::none");
        assert!(!some);
        assert!(none_payload.is_empty());

        vm.store_u64(list[2][0] + 8, 99)
            .expect("forge inactive Option payload");
        assert_eq!(
            crate::sum::read_words(&vm, list[2][0], option_layout),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn recursive_tags_reject_ambiguous_or_noncanonical_shapes() {
        let option_type = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
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
                ty: EntrypointValueTypeV1 {
                    nodes: vec![
                        EntrypointValueTypeNodeV1::Option,
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                    ],
                },
            }],
        };
        let option_schema_bytes = to_bytes(&option_schema).expect("option schema bytes");
        let hidden = EntrypointArgumentRecordV1 {
            schema_hash: entrypoint_argument_schema_hash_v1(&option_schema_bytes),
            atoms: vec![
                EntrypointValueAtomV1::Tag(false),
                EntrypointValueAtomV1::Int(99),
            ],
        };
        let mut vm = install_raw_record(&option_schema, &hidden);
        assert_eq!(decode_argument_record(&mut vm), Err(VMError::DecodeError));

        let wrong_hash = EntrypointArgumentRecordV1 {
            schema_hash: [7; 32],
            atoms: vec![EntrypointValueAtomV1::Tag(false)],
        };
        let mut vm = install_raw_record(&option_schema, &wrong_hash);
        assert_eq!(decode_argument_record(&mut vm), Err(VMError::DecodeError));

        let name_schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".into(),
                ty: argument_type(EntrypointValueKindV1::Name),
            }],
        };
        let name_schema_bytes = to_bytes(&name_schema).expect("Name schema bytes");
        let malformed_name = EntrypointArgumentRecordV1 {
            schema_hash: entrypoint_argument_schema_hash_v1(&name_schema_bytes),
            atoms: vec![EntrypointValueAtomV1::Pointer(
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
                ty: argument_type(EntrypointValueKindV1::Int),
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
    fn new_v1_kinds_reject_noncanonical_boundary_values_without_record_decode() {
        let cases = [
            (
                EntrypointValueKindV1::Bool,
                njson::Value::String("true".to_owned()),
            ),
            (EntrypointValueKindV1::String, njson::Value::Bool(true)),
            (EntrypointValueKindV1::U128, njson::Value::from(7_u64)),
            (
                EntrypointValueKindV1::U128,
                njson::Value::String("01".to_owned()),
            ),
            (
                EntrypointValueKindV1::AssetId,
                njson::Value::String("not-an-asset".to_owned()),
            ),
            (
                EntrypointValueKindV1::DomainId,
                njson::Value::String("missing_dataspace".to_owned()),
            ),
            (
                EntrypointValueKindV1::DataSpaceId,
                njson::Value::String("7".to_owned()),
            ),
            (
                EntrypointValueKindV1::Blob,
                njson::Value::String("0102".to_owned()),
            ),
            (
                EntrypointValueKindV1::Blob,
                njson::Value::String("0xAB".to_owned()),
            ),
            (
                EntrypointValueKindV1::Blob,
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
                    ty: argument_type(EntrypointValueKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "bytes".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Blob),
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

    fn assert_aggregate_schema_predecode_bound(
        schema: &EntrypointArgumentSchemaV1,
        payload: &Json,
        expected_materialized_raw_heap: u64,
    ) {
        let canonical: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(schema, payload)
                .expect("encode aggregate argument record"),
        );
        let schema_bound = schema_materialization_bound(schema).expect("aggregate schema bound");
        let bound = prepared_gas_bound(schema, canonical.len());

        reset_argument_record_decode_count();
        assert!(matches!(
            prepare_argument_record_with_gas_limit(
                schema,
                Arc::clone(&canonical),
                bound.saturating_sub(1),
            ),
            Err(VMError::OutOfGas)
        ));
        assert_eq!(argument_record_decode_count(), 0);

        let prepared = prepare_argument_record_with_gas_limit(schema, canonical, bound)
            .expect("aggregate record must fit its conservative bound");
        assert_eq!(argument_record_decode_count(), 1);
        assert!(prepared.inner.decode_plan.gas() <= bound);
        assert!(
            prepared.inner.decode_plan.materialized_bytes
                <= materialized_bytes_for_schema_bound(
                    prepared.canonical_bytes().len(),
                    schema_bound,
                ),
            "actual materialization must stay inside the schema-derived envelope"
        );
        let materialized_raw_heap = prepared.inner.decode_plan.raw_heap_bytes();
        assert_eq!(
            materialized_raw_heap, expected_materialized_raw_heap,
            "aggregate fixture must exercise the expected active raw-HEAP layout"
        );
        assert!(materialized_raw_heap <= crate::memory::Memory::HEAP_SIZE);
    }

    #[test]
    fn prepared_gas_admission_tracks_list_capacity_and_fits_the_default() {
        let schema_with_capacity = |capacity| EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: list_type(capacity, argument_type(EntrypointValueKindV1::Int)),
            }],
        };
        let narrow = schema_with_capacity(1);
        let wide = schema_with_capacity(64);
        let payload = Json::from(norito::json!({ "value": [] }));
        let narrow_record: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(&narrow, &payload).expect("encode narrow record"),
        );
        let wide_record: Arc<[u8]> = Arc::from(
            encode_argument_record_from_json(&wide, &payload).expect("encode wide record"),
        );
        let narrow_schema = to_bytes(&narrow).expect("encode narrow schema");
        let wide_schema = to_bytes(&wide).expect("encode wide schema");
        assert_eq!(narrow_record.len(), wide_record.len());
        assert_eq!(narrow_schema.len(), wide_schema.len());

        let narrow_bound = schema_materialization_bound(&narrow).expect("narrow schema bound");
        let wide_bound = schema_materialization_bound(&wide).expect("wide schema bound");
        assert_eq!(narrow_bound.raw_heap_bytes, 24);
        assert_eq!(wide_bound.raw_heap_bytes, 528);

        let narrow_quote = prepared_gas_bound(&narrow, narrow_record.len());
        let wide_quote = prepared_gas_bound(&wide, wide_record.len());
        assert_eq!(
            wide_quote - narrow_quote,
            wide_bound.raw_heap_bytes - narrow_bound.raw_heap_bytes,
            "equal wire lengths must differ only by their schema-derived List allocation"
        );
        assert!(
            wide_quote < 1_000_000,
            "a representative capacity-64 call must fit the production default"
        );
        let raw_vm = install_record(&wide, &payload);
        assert!(
            decode_argument_record_gas_quote(&raw_vm).expect("raw syscall quote") > wide_quote,
            "an unauthenticated raw syscall schema must retain its full-HEAP reserve"
        );

        reset_argument_record_decode_count();
        for (schema, record, quote) in [
            (&narrow, Arc::clone(&narrow_record), narrow_quote),
            (&wide, Arc::clone(&wide_record), wide_quote),
        ] {
            assert!(matches!(
                prepare_argument_record_with_gas_limit(schema, record, quote - 1),
                Err(VMError::OutOfGas)
            ));
        }
        assert_eq!(argument_record_decode_count(), 0);
        prepare_argument_record_with_gas_limit(&narrow, narrow_record, narrow_quote)
            .expect("the exact schema bound must cover the narrow shape");
        prepare_argument_record_with_gas_limit(&wide, wide_record, 1_000_000)
            .expect("the capacity-64 shape must fit the default cycle limit");
        assert_eq!(
            argument_record_decode_count(),
            2,
            "unaffordable rejection must precede each successful canonical record decode"
        );
    }

    #[test]
    fn aggregate_predecode_gas_bound_covers_fixed_and_nested_raw_heap_layouts() {
        let empty_string_list = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: list_type(64, argument_type(EntrypointValueKindV1::String)),
            }],
        };
        let empty_list_payload = Json::from(norito::json!({ "value": [] }));
        let empty_string_list_bound =
            schema_materialization_bound(&empty_string_list).expect("list schema bound");
        assert_eq!(empty_string_list_bound.raw_heap_bytes, 528);
        assert_eq!(
            empty_string_list_bound.pointer_envelopes, 64,
            "pointer-copy alignment allowance must scale with bounded element capacity"
        );
        assert_aggregate_schema_predecode_bound(&empty_string_list, &empty_list_payload, 528);
        assert_aggregate_schema_predecode_bound(
            &empty_string_list,
            &Json::from(norito::json!({ "value": ["a", "bb"] })),
            528,
        );

        let option_wide_tuple = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: EntrypointValueTypeV1 {
                    nodes: vec![
                        EntrypointValueTypeNodeV1::Option,
                        EntrypointValueTypeNodeV1::Tuple(4),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                    ],
                },
            }],
        };
        let none_payload = Json::from(norito::json!({ "value": { "none": true } }));
        assert_eq!(
            schema_materialization_bound(&option_wide_tuple)
                .expect("Option schema bound")
                .raw_heap_bytes,
            40
        );
        assert_aggregate_schema_predecode_bound(&option_wide_tuple, &none_payload, 40);

        let named_product = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: EntrypointValueTypeV1 {
                    nodes: vec![
                        EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                            name: "Pair".to_owned(),
                            fields: vec!["items".to_owned(), "maybe".to_owned()],
                        }),
                        EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 1 }),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Option,
                        EntrypointValueTypeNodeV1::Tuple(2),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                    ],
                },
            }],
        };
        let product_payload = Json::from(norito::json!({
            "value": { "items": [], "maybe": { "none": true } }
        }));
        assert_eq!(
            schema_materialization_bound(&named_product).expect("named product schema bound"),
            SchemaMaterializationBound {
                words: 2,
                pointer_envelopes: 0,
                raw_heap_bytes: 48,
            }
        );
        assert_aggregate_schema_predecode_bound(&named_product, &product_payload, 48);

        let result_unequal_branches = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: EntrypointValueTypeV1 {
                    nodes: vec![
                        EntrypointValueTypeNodeV1::Result,
                        EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 64 }),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
                        EntrypointValueTypeNodeV1::Tuple(4),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                    ],
                },
            }],
        };
        let err_payload = Json::from(norito::json!({ "value": { "err": [1, 2, 3, 4] } }));
        assert_eq!(
            schema_materialization_bound(&result_unequal_branches)
                .expect("Result schema bound")
                .raw_heap_bytes,
            568,
            "the fixed Result allocation plus the larger active branch must be reserved"
        );
        assert_aggregate_schema_predecode_bound(&result_unequal_branches, &err_payload, 40);

        let nested_element = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 64 }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
            ],
        };
        let nested = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: list_type(64, nested_element),
            }],
        };
        assert_eq!(
            schema_materialization_bound(&nested)
                .expect("nested schema bound")
                .raw_heap_bytes,
            35_344
        );
        assert_aggregate_schema_predecode_bound(&nested, &empty_list_payload, 528);

        reset_argument_record_decode_count();
        let mut vm = install_record(&empty_string_list, &empty_list_payload);
        let raw_quote = decode_argument_record_gas_quote(&vm).expect("quote aggregate record");
        assert_eq!(argument_record_decode_count(), 0);
        let actual = decode_argument_record(&mut vm).expect("decode aggregate record");
        assert!(actual <= raw_quote);
    }

    #[test]
    fn deep_aggregate_schema_bound_is_iterative_and_caps_at_owned_heap() {
        let levels = MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1;
        let mut nodes = Vec::with_capacity(levels.saturating_add(1));
        for _ in 0..levels {
            nodes.push(EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
                capacity: 64,
            }));
        }
        nodes.push(EntrypointValueTypeNodeV1::Leaf(
            EntrypointValueKindV1::String,
        ));
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: EntrypointValueTypeV1 { nodes },
            }],
        };
        assert!(schema.validate());
        assert_eq!(
            schema_materialization_bound(&schema)
                .expect("deep schema bound")
                .raw_heap_bytes,
            crate::memory::Memory::HEAP_SIZE,
            "exponential nested capacity must saturate at the executable HEAP ceiling"
        );
        assert_aggregate_schema_predecode_bound(
            &schema,
            &Json::from(norito::json!({ "value": [] })),
            528,
        );
    }

    #[test]
    fn malformed_schema_cannot_obtain_a_cheaper_predecode_quote() {
        let malformed = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: EntrypointValueTypeV1 {
                    nodes: vec![
                        EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 0 }),
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                    ],
                },
            }],
        };
        assert_eq!(
            schema_materialization_bound(&malformed),
            Err(VMError::DecodeError)
        );

        reset_argument_record_decode_count();
        assert!(matches!(
            prepare_argument_record_with_gas_limit(&malformed, Arc::from([]), 0),
            Err(VMError::DecodeError)
        ));
        assert_eq!(
            argument_record_decode_count(),
            0,
            "schema rejection must precede any untrusted record decode"
        );
    }

    #[test]
    fn schema_bound_arithmetic_saturates_at_the_heap_ceiling() {
        assert_eq!(
            add_pointer_envelopes(u64::MAX, u64::MAX),
            max_schema_pointer_envelopes()
        );
        assert_eq!(
            multiply_pointer_envelopes(u64::MAX, u64::MAX),
            max_schema_pointer_envelopes()
        );
        assert_eq!(aligned_allocation_bytes(usize::MAX), u64::MAX);
        assert_eq!(
            pointer_copy_allocation_upper_bound(usize::MAX, u64::MAX),
            u64::MAX
        );
        assert_eq!(
            aggregate_allocation_bytes(u64::MAX, u64::MAX),
            crate::memory::Memory::HEAP_SIZE
        );
        assert_eq!(
            add_raw_heap_bytes(u64::MAX, u64::MAX),
            crate::memory::Memory::HEAP_SIZE
        );
        assert_eq!(
            multiply_raw_heap_bytes(u64::MAX, u64::MAX),
            crate::memory::Memory::HEAP_SIZE
        );
        assert_eq!(
            argument_record_gas_for_schema_bound(
                usize::MAX,
                usize::MAX,
                SchemaMaterializationBound {
                    words: u64::MAX,
                    pointer_envelopes: u64::MAX,
                    raw_heap_bytes: u64::MAX,
                },
            ),
            u64::MAX
        );
    }

    #[test]
    fn gas_quote_is_conservative_repeatable_and_side_effect_free() {
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

        let actual = decode_argument_record(&mut quoted_vm).expect("execute valid record");
        assert!(
            actual <= quote,
            "the pre-decode reserve must bound the exact post-decode cost"
        );
        assert!(
            actual < quote,
            "this pointer-bearing fixture must exercise conservative-reserve refunding"
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
    fn unaffordable_malformed_envelopes_fail_before_digest_or_canonical_decode() {
        RECORD_DECODE_COUNT.with(|count| count.set(0));
        let mut vm = IVM::new(u64::MAX);
        let mut record_envelope = encode_tlv(PointerType::NoritoBytes, b"not a record")
            .expect("encode malformed record envelope");
        *record_envelope.last_mut().expect("record has a digest") ^= 1;
        let mut schema_envelope = encode_tlv(PointerType::NoritoBytes, b"not a schema")
            .expect("encode malformed schema envelope");
        *schema_envelope.last_mut().expect("schema has a digest") ^= 1;
        let record_ptr = vm
            .alloc_input_tlv(&record_envelope)
            .expect("allocate malformed record envelope");
        let schema_ptr = vm
            .alloc_input_tlv(&schema_envelope)
            .expect("allocate malformed schema envelope");
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        let quote = decode_argument_record_gas_quote(&vm)
            .expect("bounded envelope shapes receive a conservative quote");

        let mut program = crate::metadata::ProgramMetadata::default().encode();
        program.extend_from_slice(
            &crate::encoding::wide::encode_syscallx(
                ivm_abi::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD,
            )
            .to_le_bytes(),
        );
        program.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
        vm.load_program(&program).expect("load argument syscall");
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);
        vm.set_host(crate::host::DefaultHost::new());
        vm.set_gas_limit(5_u64.saturating_add(quote).saturating_sub(1));

        assert_eq!(
            vm.run(),
            Err(VMError::OutOfGas),
            "gas debit must reject the call before envelope authentication"
        );
        RECORD_DECODE_COUNT.with(|count| {
            assert_eq!(
                count.get(),
                0,
                "unaffordable malformed calls must not reach canonical decoding"
            );
        });
        assert_eq!(vm.register(10), record_ptr);
        assert_eq!(vm.register(11), schema_ptr);
    }

    #[test]
    fn gas_quote_does_not_decode_invalid_schema_before_debit() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "same".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "same".to_owned(),
                    ty: argument_type(EntrypointValueKindV1::Blob),
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
            .expect("bounded envelope lengths are sufficient to quote");
        assert!(quote > 0);
        assert_eq!((vm.register(10), vm.register(11)), before);
        assert_eq!(
            decode_argument_record(&mut vm),
            Err(VMError::DecodeError),
            "schema validation belongs to post-debit execution"
        );
    }

    #[test]
    fn gas_quote_does_not_authenticate_envelope_digest_before_debit() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".to_owned(),
                ty: argument_type(EntrypointValueKindV1::Int),
            }],
        };
        let record =
            encode_argument_record_from_json(&schema, &Json::from(norito::json!({"value": 1})))
                .expect("encode record");
        let mut record_envelope =
            encode_tlv(PointerType::NoritoBytes, &record).expect("encode record envelope");
        *record_envelope.last_mut().expect("envelope has a digest") ^= 1;

        let mut vm = IVM::new(u64::MAX);
        let record_ptr = vm
            .alloc_input_tlv(&record_envelope)
            .expect("allocate corrupted record envelope");
        let schema_ptr = alloc(
            &mut vm,
            PointerType::NoritoBytes,
            &to_bytes(&schema).expect("encode schema"),
        );
        vm.set_register(10, record_ptr);
        vm.set_register(11, schema_ptr);

        assert!(
            decode_argument_record_gas_quote(&vm).is_ok(),
            "header and bounded lengths are sufficient for pre-debit quoting"
        );
        assert_eq!(
            decode_argument_record(&mut vm),
            Err(VMError::NoritoInvalid),
            "digest authentication belongs to post-debit execution"
        );
    }

    #[test]
    fn gas_quote_arithmetic_saturates_instead_of_wrapping() {
        let plan = ArgumentDecodePlan {
            decoded: Vec::new(),
            roots: Vec::new(),
            record_bytes: usize::MAX,
            schema_bytes: usize::MAX,
            materialized_bytes: u64::MAX,
        };
        assert_eq!(plan.gas(), u64::MAX);
    }

    #[test]
    fn quote_rejects_oversized_envelopes_without_decoding_them() {
        RECORD_DECODE_COUNT.with(|count| count.set(0));
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".into(),
                ty: argument_type(EntrypointValueKindV1::Int),
            }],
        };
        let oversized: Arc<[u8]> = Arc::from(vec![0_u8; MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES + 1]);
        assert!(matches!(
            prepare_argument_record_with_gas_limit(&schema, oversized, u64::MAX),
            Err(VMError::DecodeError)
        ));
        RECORD_DECODE_COUNT.with(|count| assert_eq!(count.get(), 0));
    }
}
