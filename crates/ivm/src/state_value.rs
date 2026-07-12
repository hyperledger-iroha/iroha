//! Compiler-internal canonical codec for aggregate durable-state values.

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    prelude::{AssetDefinitionId, AssetId, DataSpaceId, DomainId, Name, NftId},
    soracloud::{SoracloudHostRequestEnvelopeV1, SoracloudHostResponseEnvelopeV1},
};
use iroha_primitives::{
    json::Json,
    numeric_abi::{DecimalValueV1, IntValueV1, QuantityValueV1},
};
use ivm_abi::list::ListLayoutV1;
use ivm_abi::state_value::{
    MAX_STATE_VALUE_RECORD_BYTES, MAX_STATE_VALUE_SCHEMA_BYTES, MAX_STATE_VALUE_WORDS,
    StateValueAtomV1, StateValueKindV1, StateValueNodeV1, StateValueRecordV1, StateValueSchemaV1,
    state_value_schema_hash_v1,
};
use ivm_abi::sum::SumLayoutV1;
use norito::{decode_from_bytes, to_bytes};

use crate::{
    VMError,
    host::preflight_reserved_syscall_gas,
    ivm::IVM,
    pointer_abi::{self, PointerType, Tlv},
};

const STATE_VALUE_GAS_BASE: u64 = 32;

type AddressResolver = fn(&IVM, u64) -> u64;

fn gas(bytes: usize, words: usize) -> u64 {
    STATE_VALUE_GAS_BASE
        .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(words).unwrap_or(u64::MAX))
}

fn load_tlv<'a>(
    vm: &'a IVM,
    address: u64,
    resolver: AddressResolver,
) -> Result<(&'a [u8], Tlv<'a>), VMError> {
    let original_address = address;
    let address = resolver(vm, address);
    let tlv = vm.validate_tlv(address).inspect_err(|error| {
        if crate::dev_env::decode_trace_enabled() {
            eprintln!(
                "[state-value] invalid TLV pointer original=0x{original_address:016x} resolved=0x{address:016x}: {error:?}"
            );
        }
    })?;
    let total = 7usize
        .checked_add(tlv.payload.len())
        .and_then(|len| len.checked_add(Hash::LENGTH))
        .ok_or(VMError::NoritoInvalid)?;
    let envelope = vm.memory.load_region(
        address,
        u64::try_from(total).map_err(|_| VMError::NoritoInvalid)?,
    )?;
    Ok((envelope, tlv))
}

fn load_expected_tlv<'a>(
    vm: &'a IVM,
    address: u64,
    expected: PointerType,
    resolver: AddressResolver,
) -> Result<(&'a [u8], Tlv<'a>), VMError> {
    let (envelope, tlv) = load_tlv(vm, address, resolver)?;
    if tlv.type_id != expected {
        if crate::dev_env::decode_trace_enabled() {
            eprintln!(
                "[state-value] TLV type mismatch pointer=0x{address:016x} expected={expected:?} actual={:?}",
                tlv.type_id
            );
        }
        return Err(VMError::NoritoInvalid);
    }
    Ok((envelope, tlv))
}

fn decode_schema(
    vm: &IVM,
    address: u64,
    resolver: AddressResolver,
) -> Result<(StateValueSchemaV1, &[u8]), VMError> {
    let (_, tlv) = load_expected_tlv(vm, address, PointerType::NoritoBytes, resolver)?;
    if tlv.payload.len() > MAX_STATE_VALUE_SCHEMA_BYTES {
        return Err(VMError::NoritoInvalid);
    }
    let schema: StateValueSchemaV1 =
        decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
    if !schema.validate()
        || to_bytes(&schema)
            .map_err(|_| VMError::DecodeError)?
            .as_slice()
            != tlv.payload
    {
        return Err(VMError::DecodeError);
    }
    Ok((schema, tlv.payload))
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

fn validate_pointer_payload(kind: StateValueKindV1, payload: &[u8]) -> Result<(), VMError> {
    match kind {
        StateValueKindV1::Bool => return Err(VMError::DecodeError),
        StateValueKindV1::Int => {
            IntValueV1::decode_frame(payload).map_err(|_| VMError::DecodeError)?;
        }
        StateValueKindV1::Decimal => {
            DecimalValueV1::decode_frame(payload).map_err(|_| VMError::DecodeError)?;
        }
        StateValueKindV1::Quantity => {
            QuantityValueV1::decode_frame(payload).map_err(|_| VMError::DecodeError)?;
        }
        StateValueKindV1::String => {
            std::str::from_utf8(payload).map_err(|_| VMError::DecodeError)?;
        }
        StateValueKindV1::Json => {
            let _: Json = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::Bytes => {}
        StateValueKindV1::AccountId => {
            let _: AccountId = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::AssetDefinitionId => {
            let _: AssetDefinitionId = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::AssetId => {
            let _: AssetId = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::DomainId => {
            let _: DomainId = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::NftId => {
            let _: NftId = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::Name => {
            let _: Name = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::DataSpaceId => {
            let _: DataSpaceId = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::AxtDescriptor => {
            let value: crate::axt::AxtDescriptor = decode_canonical_norito(payload)?;
            crate::axt::validate_descriptor(&value)?;
        }
        StateValueKindV1::AssetHandle => {
            let _: crate::axt::AssetHandle = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::ProofBlob => {
            let _: crate::axt::ProofBlob = decode_canonical_norito(payload)?;
        }
        StateValueKindV1::SoracloudRequest => {
            let value: SoracloudHostRequestEnvelopeV1 = decode_canonical_norito(payload)?;
            value.validate().map_err(|_| VMError::DecodeError)?;
        }
        StateValueKindV1::SoracloudResponse => {
            let value: SoracloudHostResponseEnvelopeV1 = decode_canonical_norito(payload)?;
            value.validate().map_err(|_| VMError::DecodeError)?;
        }
    }
    Ok(())
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

fn table_words(vm: &IVM, address: u64, count: usize) -> Result<Vec<u64>, VMError> {
    if count > MAX_STATE_VALUE_WORDS || !address.is_multiple_of(8) {
        return Err(VMError::NoritoInvalid);
    }
    let byte_len = count.checked_mul(8).ok_or(VMError::NoritoInvalid)?;
    if byte_len == 0 {
        return Ok(Vec::new());
    }
    vm.ensure_public_memory(
        address,
        u64::try_from(byte_len).map_err(|_| VMError::NoritoInvalid)?,
    )?;
    let bytes = vm.memory.load_region(
        address,
        u64::try_from(byte_len).map_err(|_| VMError::NoritoInvalid)?,
    )?;
    Ok(bytes
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("eight-byte chunk")))
        .collect())
}

fn skip_state_node(nodes: &[StateValueNodeV1], node_index: &mut usize) -> Result<(), VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Struct { fields, .. } => {
            for _ in fields {
                skip_state_node(nodes, node_index)?;
            }
        }
        StateValueNodeV1::Tuple { arity } => {
            for _ in 0..*arity {
                skip_state_node(nodes, node_index)?;
            }
        }
        StateValueNodeV1::Option => skip_state_node(nodes, node_index)?,
        StateValueNodeV1::Result => {
            skip_state_node(nodes, node_index)?;
            skip_state_node(nodes, node_index)?;
        }
        StateValueNodeV1::List { .. } | StateValueNodeV1::Leaf(_) => {}
    }
    Ok(())
}

fn state_node_word_count(
    nodes: &[StateValueNodeV1],
    node_index: &mut usize,
) -> Result<usize, VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Struct { fields, .. } => {
            let mut words = 0_usize;
            for _ in fields {
                words = words
                    .checked_add(state_node_word_count(nodes, node_index)?)
                    .ok_or(VMError::DecodeError)?;
            }
            Ok(words)
        }
        StateValueNodeV1::Tuple { arity } => {
            let mut words = 0_usize;
            for _ in 0..*arity {
                words = words
                    .checked_add(state_node_word_count(nodes, node_index)?)
                    .ok_or(VMError::DecodeError)?;
            }
            Ok(words)
        }
        StateValueNodeV1::Option => {
            state_node_word_count(nodes, node_index)?;
            Ok(1)
        }
        StateValueNodeV1::Result => {
            state_node_word_count(nodes, node_index)?;
            state_node_word_count(nodes, node_index)?;
            Ok(1)
        }
        StateValueNodeV1::List { .. } | StateValueNodeV1::Leaf(_) => Ok(1),
    }
}

fn validate_state_pointer_atom(
    policy: ivm_abi::SyscallPolicy,
    kind: StateValueKindV1,
    envelope: &[u8],
) -> Result<(), VMError> {
    let expected = kind.pointer_type().ok_or(VMError::DecodeError)?;
    let tlv = pointer_abi::validate_tlv_bytes(envelope)?;
    if tlv.type_id != expected || !pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id) {
        return Err(VMError::DecodeError);
    }
    // Exact envelope validation above has already authenticated these bytes;
    // rebuilding the same TLV would only repeat the payload hash.
    validate_pointer_payload(kind, tlv.payload)
}

fn validate_state_atoms_recursive(
    policy: ivm_abi::SyscallPolicy,
    nodes: &[StateValueNodeV1],
    atoms: &[StateValueAtomV1],
    node_index: &mut usize,
    atom_index: &mut usize,
) -> Result<(), VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Struct { fields, .. } => {
            for _ in fields {
                validate_state_atoms_recursive(policy, nodes, atoms, node_index, atom_index)?;
            }
        }
        StateValueNodeV1::Tuple { arity } => {
            for _ in 0..*arity {
                validate_state_atoms_recursive(policy, nodes, atoms, node_index, atom_index)?;
            }
        }
        StateValueNodeV1::Option => {
            let StateValueAtomV1::Tag(tag) = atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            *atom_index = atom_index.saturating_add(1);
            if *tag {
                validate_state_atoms_recursive(policy, nodes, atoms, node_index, atom_index)?;
            } else {
                skip_state_node(nodes, node_index)?;
            }
        }
        StateValueNodeV1::Result => {
            let StateValueAtomV1::Tag(tag) = atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            *atom_index = atom_index.saturating_add(1);
            if *tag {
                validate_state_atoms_recursive(policy, nodes, atoms, node_index, atom_index)?;
                skip_state_node(nodes, node_index)?;
            } else {
                skip_state_node(nodes, node_index)?;
                validate_state_atoms_recursive(policy, nodes, atoms, node_index, atom_index)?;
            }
        }
        StateValueNodeV1::List { element, capacity } => {
            let StateValueAtomV1::List(items) =
                atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            *atom_index = atom_index.saturating_add(1);
            if items.len() > usize::from(*capacity) {
                return Err(VMError::DecodeError);
            }
            for item in items {
                let mut item_node = 0;
                let mut item_atom = 0;
                validate_state_atoms_recursive(
                    policy,
                    &element.nodes,
                    item,
                    &mut item_node,
                    &mut item_atom,
                )?;
                if item_node != element.nodes.len() || item_atom != item.len() {
                    return Err(VMError::DecodeError);
                }
            }
        }
        StateValueNodeV1::Leaf(StateValueKindV1::Bool) => {
            if !matches!(atoms.get(*atom_index), Some(StateValueAtomV1::Bool(_))) {
                return Err(VMError::DecodeError);
            }
            *atom_index = atom_index.saturating_add(1);
        }
        StateValueNodeV1::Leaf(kind) => {
            let StateValueAtomV1::Pointer(envelope) =
                atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            validate_state_pointer_atom(policy, *kind, envelope)?;
            *atom_index = atom_index.saturating_add(1);
        }
    }
    Ok(())
}

fn validate_state_atom_stream(
    policy: ivm_abi::SyscallPolicy,
    schema: &StateValueSchemaV1,
    atoms: &[StateValueAtomV1],
) -> Result<(), VMError> {
    let mut node_index = 0;
    let mut atom_index = 0;
    validate_state_atoms_recursive(
        policy,
        &schema.nodes,
        atoms,
        &mut node_index,
        &mut atom_index,
    )?;
    if node_index != schema.nodes.len() || atom_index != atoms.len() {
        return Err(VMError::DecodeError);
    }
    Ok(())
}

fn append_embedded_state_schema_nodes(
    ty: &crate::metadata::EmbeddedStateType,
    nodes: &mut Vec<StateValueNodeV1>,
) -> Result<(), VMError> {
    use crate::metadata::EmbeddedStateType as Embedded;
    use StateValueKindV1 as Kind;

    match ty {
        Embedded::Int => nodes.push(StateValueNodeV1::Leaf(Kind::Int)),
        Embedded::Decimal => nodes.push(StateValueNodeV1::Leaf(Kind::Decimal)),
        Embedded::Quantity => nodes.push(StateValueNodeV1::Leaf(Kind::Quantity)),
        Embedded::Bool => nodes.push(StateValueNodeV1::Leaf(Kind::Bool)),
        Embedded::String => nodes.push(StateValueNodeV1::Leaf(Kind::String)),
        Embedded::Bytes => nodes.push(StateValueNodeV1::Leaf(Kind::Bytes)),
        Embedded::DataSpaceId => nodes.push(StateValueNodeV1::Leaf(Kind::DataSpaceId)),
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
            let arity = u16::try_from(items.len()).map_err(|_| VMError::NoritoInvalid)?;
            nodes.push(StateValueNodeV1::Tuple { arity });
            for item in items {
                append_embedded_state_schema_nodes(item, nodes)?;
            }
        }
        Embedded::Struct { name, fields } => {
            nodes.push(StateValueNodeV1::Struct {
                name: name.clone(),
                fields: fields.iter().map(|field| field.name.clone()).collect(),
            });
            for field in fields {
                append_embedded_state_schema_nodes(&field.ty, nodes)?;
            }
        }
        Embedded::Option(inner) => {
            nodes.push(StateValueNodeV1::Option);
            append_embedded_state_schema_nodes(inner, nodes)?;
        }
        Embedded::Result { ok, err } => {
            nodes.push(StateValueNodeV1::Result);
            append_embedded_state_schema_nodes(ok, nodes)?;
            append_embedded_state_schema_nodes(err, nodes)?;
        }
        Embedded::List { element, capacity } => {
            let mut element_nodes = Vec::new();
            append_embedded_state_schema_nodes(element, &mut element_nodes)?;
            let element = StateValueSchemaV1 {
                nodes: element_nodes,
            };
            if !element.validate() {
                return Err(VMError::NoritoInvalid);
            }
            nodes.push(StateValueNodeV1::List {
                element: Box::new(element),
                capacity: *capacity,
            });
        }
        Embedded::StateMap { .. } => return Err(VMError::NoritoInvalid),
    }
    Ok(())
}

/// Reconstruct the exact compiler-owned durable-value schema embedded in CNTR.
pub(crate) fn schema_for_embedded_state_type(
    ty: &crate::metadata::EmbeddedStateType,
) -> Result<StateValueSchemaV1, VMError> {
    let mut nodes = Vec::new();
    append_embedded_state_schema_nodes(ty, &mut nodes)?;
    let schema = StateValueSchemaV1 { nodes };
    if !schema.validate() {
        return Err(VMError::NoritoInvalid);
    }
    Ok(schema)
}

fn decode_validated_state_value_record(
    policy: ivm_abi::SyscallPolicy,
    schema: &StateValueSchemaV1,
    schema_payload: &[u8],
    payload: &[u8],
) -> Result<StateValueRecordV1, VMError> {
    if !schema.validate()
        || schema_payload.len() > MAX_STATE_VALUE_SCHEMA_BYTES
        || payload.len() > MAX_STATE_VALUE_RECORD_BYTES
    {
        return Err(VMError::NoritoInvalid);
    }
    let record: StateValueRecordV1 =
        decode_from_bytes(payload).map_err(|_| VMError::DecodeError)?;
    if record.schema_hash != state_value_schema_hash_v1(schema_payload)
        || to_bytes(&record)
            .map_err(|_| VMError::DecodeError)?
            .as_slice()
            != payload
    {
        return Err(VMError::DecodeError);
    }
    validate_state_atom_stream(policy, schema, &record.atoms)?;
    Ok(record)
}

/// Validate one persisted record against an exact CNTR-derived state schema.
pub(crate) fn validate_state_value_record(
    vm: &IVM,
    schema: &StateValueSchemaV1,
    payload: &[u8],
) -> Result<(), VMError> {
    let schema_payload = to_bytes(schema).map_err(|_| VMError::DecodeError)?;
    decode_validated_state_value_record(vm.syscall_policy(), schema, &schema_payload, payload)
        .map(drop)
}

#[derive(Clone, Copy)]
struct StateEncodeContext<'a> {
    vm: &'a IVM,
    resolver: AddressResolver,
}

impl StateEncodeContext<'_> {
    fn resolve(self, pointer: u64) -> u64 {
        (self.resolver)(self.vm, pointer)
    }
}

fn encode_state_node(
    nodes: &[StateValueNodeV1],
    node_index: &mut usize,
    words: &[u64],
    word_index: &mut usize,
    context: StateEncodeContext<'_>,
    atoms: &mut Vec<StateValueAtomV1>,
    pointer_bytes: &mut usize,
) -> Result<(), VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Struct { fields, .. } => {
            for _ in fields {
                encode_state_node(
                    nodes,
                    node_index,
                    words,
                    word_index,
                    context,
                    atoms,
                    pointer_bytes,
                )?;
            }
        }
        StateValueNodeV1::Tuple { arity } => {
            for _ in 0..*arity {
                encode_state_node(
                    nodes,
                    node_index,
                    words,
                    word_index,
                    context,
                    atoms,
                    pointer_bytes,
                )?;
            }
        }
        StateValueNodeV1::Option => {
            let pointer = *words.get(*word_index).ok_or(VMError::DecodeError)?;
            *word_index = word_index.saturating_add(1);
            if pointer == 0 {
                return Err(VMError::DecodeError);
            }
            let mut child_end = *node_index;
            let child_words = state_node_word_count(nodes, &mut child_end)?;
            let layout =
                SumLayoutV1::option(u64::try_from(child_words).map_err(|_| VMError::DecodeError)?)
                    .map_err(|_| VMError::DecodeError)?;
            let (tag, payload) =
                crate::sum::read_words(context.vm, context.resolve(pointer), layout)?;
            atoms.push(StateValueAtomV1::Tag(tag));
            let mut active_word = 0;
            if tag {
                encode_state_node(
                    nodes,
                    node_index,
                    &payload,
                    &mut active_word,
                    context,
                    atoms,
                    pointer_bytes,
                )?;
            } else {
                skip_state_node(nodes, node_index)?;
            }
            if *node_index != child_end || active_word != payload.len() {
                return Err(VMError::DecodeError);
            }
            *pointer_bytes = pointer_bytes.saturating_add(
                usize::try_from(
                    layout
                        .allocation_bytes()
                        .map_err(|_| VMError::DecodeError)?,
                )
                .unwrap_or(usize::MAX),
            );
        }
        StateValueNodeV1::Result => {
            let pointer = *words.get(*word_index).ok_or(VMError::DecodeError)?;
            *word_index = word_index.saturating_add(1);
            if pointer == 0 {
                return Err(VMError::DecodeError);
            }
            let mut ok_end = *node_index;
            let ok_words = state_node_word_count(nodes, &mut ok_end)?;
            let mut err_end = ok_end;
            let err_words = state_node_word_count(nodes, &mut err_end)?;
            let layout = SumLayoutV1::try_new(
                u64::try_from(err_words).map_err(|_| VMError::DecodeError)?,
                u64::try_from(ok_words).map_err(|_| VMError::DecodeError)?,
            )
            .map_err(|_| VMError::DecodeError)?;
            let (tag, payload) =
                crate::sum::read_words(context.vm, context.resolve(pointer), layout)?;
            atoms.push(StateValueAtomV1::Tag(tag));
            let mut active_word = 0;
            if tag {
                encode_state_node(
                    nodes,
                    node_index,
                    &payload,
                    &mut active_word,
                    context,
                    atoms,
                    pointer_bytes,
                )?;
                skip_state_node(nodes, node_index)?;
            } else {
                skip_state_node(nodes, node_index)?;
                encode_state_node(
                    nodes,
                    node_index,
                    &payload,
                    &mut active_word,
                    context,
                    atoms,
                    pointer_bytes,
                )?;
            }
            if *node_index != err_end || active_word != payload.len() {
                return Err(VMError::DecodeError);
            }
            *pointer_bytes = pointer_bytes.saturating_add(
                usize::try_from(
                    layout
                        .allocation_bytes()
                        .map_err(|_| VMError::DecodeError)?,
                )
                .unwrap_or(usize::MAX),
            );
        }
        StateValueNodeV1::List { element, capacity } => {
            let pointer = *words.get(*word_index).ok_or(VMError::DecodeError)?;
            *word_index = word_index.saturating_add(1);
            if pointer == 0 {
                return Err(VMError::DecodeError);
            }
            let element_words = element.word_count().ok_or(VMError::DecodeError)?;
            let layout = ListLayoutV1::try_new(
                u64::from(*capacity),
                u64::try_from(element_words).map_err(|_| VMError::DecodeError)?,
            )
            .map_err(|_| VMError::DecodeError)?;
            let raw_items = crate::list::read_words(context.vm, context.resolve(pointer), layout)?;
            let mut items = Vec::with_capacity(raw_items.len());
            for words in raw_items {
                let mut item = Vec::new();
                let mut item_node = 0;
                let mut item_word = 0;
                encode_state_node(
                    &element.nodes,
                    &mut item_node,
                    &words,
                    &mut item_word,
                    context,
                    &mut item,
                    pointer_bytes,
                )?;
                if item_node != element.nodes.len() || item_word != words.len() {
                    return Err(VMError::DecodeError);
                }
                validate_state_atom_stream(context.vm.syscall_policy(), element, &item)?;
                items.push(item);
            }
            *pointer_bytes = pointer_bytes.saturating_add(
                usize::try_from(
                    layout
                        .allocation_bytes()
                        .map_err(|_| VMError::DecodeError)?,
                )
                .unwrap_or(usize::MAX),
            );
            atoms.push(StateValueAtomV1::List(items));
        }
        StateValueNodeV1::Leaf(StateValueKindV1::Bool) => {
            let word = *words.get(*word_index).ok_or(VMError::DecodeError)?;
            *word_index = word_index.saturating_add(1);
            let value = match word {
                0 => false,
                1 => true,
                _ => return Err(VMError::DecodeError),
            };
            atoms.push(StateValueAtomV1::Bool(value));
        }
        StateValueNodeV1::Leaf(kind) => {
            let pointer = *words.get(*word_index).ok_or(VMError::DecodeError)?;
            *word_index = word_index.saturating_add(1);
            if pointer == 0 {
                return Err(VMError::DecodeError);
            }
            let expected = kind.pointer_type().ok_or(VMError::DecodeError)?;
            let (envelope, tlv) =
                load_expected_tlv(context.vm, pointer, expected, context.resolver)?;
            validate_pointer_payload(*kind, tlv.payload)?;
            *pointer_bytes = pointer_bytes.saturating_add(envelope.len());
            atoms.push(StateValueAtomV1::Pointer(envelope.to_vec()));
        }
    }
    Ok(())
}

/// Encode the compiler word table selected by `r11`/`r12` using the schema in `r10`.
pub(crate) fn encode_state_value(vm: &mut IVM, resolver: AddressResolver) -> Result<u64, VMError> {
    let (schema, schema_payload) = decode_schema(vm, vm.register(10), resolver)?;
    let count = usize::try_from(vm.register(12)).map_err(|_| VMError::NoritoInvalid)?;
    if count != schema.word_count().ok_or(VMError::DecodeError)? {
        return Err(VMError::DecodeError);
    }
    let words = table_words(vm, vm.register(11), count)?;
    let mut atoms = Vec::with_capacity(count);
    let mut pointer_bytes = 0_usize;
    let mut node_index = 0;
    let mut word_index = 0;
    encode_state_node(
        &schema.nodes,
        &mut node_index,
        &words,
        &mut word_index,
        StateEncodeContext { vm, resolver },
        &mut atoms,
        &mut pointer_bytes,
    )?;
    if node_index != schema.nodes.len()
        || word_index != words.len()
        || !schema.validate_atoms(&atoms)
    {
        return Err(VMError::DecodeError);
    }
    let record = StateValueRecordV1 {
        schema_hash: state_value_schema_hash_v1(schema_payload),
        atoms,
    };
    let payload = to_bytes(&record).map_err(|_| VMError::NoritoInvalid)?;
    if payload.len() > MAX_STATE_VALUE_RECORD_BYTES {
        return Err(VMError::NoritoInvalid);
    }
    let actual = gas(
        schema_payload
            .len()
            .saturating_add(count.saturating_mul(8))
            .saturating_add(pointer_bytes)
            .saturating_add(payload.len()),
        count,
    );
    preflight_reserved_syscall_gas(vm, actual)?;
    let out = encode_tlv(PointerType::NoritoBytes, &payload)?;
    let pointer = vm.alloc_host_tlv(&out)?;
    vm.set_register(10, pointer);
    Ok(actual)
}

enum PlannedStateWord {
    Scalar(u64),
    Pointer(Vec<u8>),
    Sum {
        layout: SumLayoutV1,
        tag: u64,
        active: Vec<PlannedStateWord>,
    },
    List {
        layout: ListLayoutV1,
        elements: Vec<Vec<PlannedStateWord>>,
    },
}

fn plan_state_atoms(
    nodes: &[StateValueNodeV1],
    atoms: &[StateValueAtomV1],
    node_index: &mut usize,
    atom_index: &mut usize,
    planned: &mut Vec<PlannedStateWord>,
) -> Result<(), VMError> {
    let node = nodes.get(*node_index).ok_or(VMError::DecodeError)?;
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Struct { fields, .. } => {
            for _ in fields {
                plan_state_atoms(nodes, atoms, node_index, atom_index, planned)?;
            }
        }
        StateValueNodeV1::Tuple { arity } => {
            for _ in 0..*arity {
                plan_state_atoms(nodes, atoms, node_index, atom_index, planned)?;
            }
        }
        StateValueNodeV1::Option => {
            let StateValueAtomV1::Tag(tag) = atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            *atom_index = atom_index.saturating_add(1);
            let mut child_end = *node_index;
            let child_words = state_node_word_count(nodes, &mut child_end)?;
            let mut active = Vec::with_capacity(child_words);
            if *tag {
                plan_state_atoms(nodes, atoms, node_index, atom_index, &mut active)?;
            } else {
                skip_state_node(nodes, node_index)?;
            }
            if *node_index != child_end || active.len() != if *tag { child_words } else { 0 } {
                return Err(VMError::DecodeError);
            }
            let layout =
                SumLayoutV1::option(u64::try_from(child_words).map_err(|_| VMError::DecodeError)?)
                    .map_err(|_| VMError::DecodeError)?;
            planned.push(PlannedStateWord::Sum {
                layout,
                tag: u64::from(*tag),
                active,
            });
        }
        StateValueNodeV1::Result => {
            let StateValueAtomV1::Tag(tag) = atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            *atom_index = atom_index.saturating_add(1);
            let mut ok_end = *node_index;
            let ok_words = state_node_word_count(nodes, &mut ok_end)?;
            let mut err_end = ok_end;
            let err_words = state_node_word_count(nodes, &mut err_end)?;
            let mut active = Vec::with_capacity(if *tag { ok_words } else { err_words });
            if *tag {
                plan_state_atoms(nodes, atoms, node_index, atom_index, &mut active)?;
                skip_state_node(nodes, node_index)?;
            } else {
                skip_state_node(nodes, node_index)?;
                plan_state_atoms(nodes, atoms, node_index, atom_index, &mut active)?;
            }
            let expected_words = if *tag { ok_words } else { err_words };
            if *node_index != err_end || active.len() != expected_words {
                return Err(VMError::DecodeError);
            }
            let layout = SumLayoutV1::try_new(
                u64::try_from(err_words).map_err(|_| VMError::DecodeError)?,
                u64::try_from(ok_words).map_err(|_| VMError::DecodeError)?,
            )
            .map_err(|_| VMError::DecodeError)?;
            planned.push(PlannedStateWord::Sum {
                layout,
                tag: u64::from(*tag),
                active,
            });
        }
        StateValueNodeV1::List { element, capacity } => {
            let StateValueAtomV1::List(items) =
                atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            *atom_index = atom_index.saturating_add(1);
            let element_words = element.word_count().ok_or(VMError::DecodeError)?;
            let layout = ListLayoutV1::try_new(
                u64::from(*capacity),
                u64::try_from(element_words).map_err(|_| VMError::DecodeError)?,
            )
            .map_err(|_| VMError::DecodeError)?;
            let mut elements = Vec::with_capacity(items.len());
            for item in items {
                let mut item_node = 0;
                let mut item_atom = 0;
                let mut values = Vec::new();
                plan_state_atoms(
                    &element.nodes,
                    item,
                    &mut item_node,
                    &mut item_atom,
                    &mut values,
                )?;
                if item_node != element.nodes.len()
                    || item_atom != item.len()
                    || values.len() != element_words
                {
                    return Err(VMError::DecodeError);
                }
                elements.push(values);
            }
            planned.push(PlannedStateWord::List { layout, elements });
        }
        StateValueNodeV1::Leaf(StateValueKindV1::Bool) => {
            let StateValueAtomV1::Bool(value) =
                atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            *atom_index = atom_index.saturating_add(1);
            planned.push(PlannedStateWord::Scalar(u64::from(*value)));
        }
        StateValueNodeV1::Leaf(_) => {
            let StateValueAtomV1::Pointer(envelope) =
                atoms.get(*atom_index).ok_or(VMError::DecodeError)?
            else {
                return Err(VMError::DecodeError);
            };
            *atom_index = atom_index.saturating_add(1);
            planned.push(PlannedStateWord::Pointer(envelope.clone()));
        }
    }
    Ok(())
}

fn planned_state_bytes(value: &PlannedStateWord) -> usize {
    match value {
        PlannedStateWord::Scalar(_) => 0,
        PlannedStateWord::Pointer(envelope) => envelope.len(),
        PlannedStateWord::Sum { layout, active, .. } => {
            active.iter().map(planned_state_bytes).fold(
                layout
                    .allocation_bytes()
                    .ok()
                    .and_then(|bytes| usize::try_from(bytes).ok())
                    .unwrap_or(usize::MAX),
                usize::saturating_add,
            )
        }
        PlannedStateWord::List { layout, elements } => {
            elements.iter().flatten().map(planned_state_bytes).fold(
                layout
                    .allocation_bytes()
                    .ok()
                    .and_then(|bytes| usize::try_from(bytes).ok())
                    .unwrap_or(usize::MAX),
                usize::saturating_add,
            )
        }
    }
}

fn planned_state_allocation_shape(value: &PlannedStateWord, tlv_lengths: &mut Vec<usize>) -> usize {
    match value {
        PlannedStateWord::Scalar(_) => 0,
        PlannedStateWord::Pointer(envelope) => {
            tlv_lengths.push(envelope.len());
            0
        }
        PlannedStateWord::Sum { layout, active, .. } => active.iter().fold(
            layout
                .allocation_bytes()
                .ok()
                .and_then(|bytes| usize::try_from(bytes).ok())
                .unwrap_or(usize::MAX),
            |bytes, value| bytes.saturating_add(planned_state_allocation_shape(value, tlv_lengths)),
        ),
        PlannedStateWord::List { layout, elements } => elements.iter().flatten().fold(
            layout
                .allocation_bytes()
                .ok()
                .and_then(|bytes| usize::try_from(bytes).ok())
                .unwrap_or(usize::MAX),
            |bytes, value| bytes.saturating_add(planned_state_allocation_shape(value, tlv_lengths)),
        ),
    }
}

fn materialize_state_word(vm: &mut IVM, value: PlannedStateWord) -> Result<u64, VMError> {
    match value {
        PlannedStateWord::Scalar(value) => Ok(value),
        PlannedStateWord::Pointer(envelope) => vm.alloc_host_tlv(&envelope),
        PlannedStateWord::Sum {
            layout,
            tag,
            active,
        } => {
            let mut payload = Vec::with_capacity(active.len());
            for value in active {
                payload.push(materialize_state_word(vm, value)?);
            }
            crate::sum::allocate_words(vm, layout, tag, &payload)
        }
        PlannedStateWord::List { layout, elements } => {
            let width =
                usize::try_from(layout.element_words()).map_err(|_| VMError::DecodeError)?;
            let mut words = Vec::with_capacity(elements.len());
            for element in elements {
                let mut item = Vec::with_capacity(width);
                for value in element {
                    item.push(materialize_state_word(vm, value)?);
                }
                if item.len() != width {
                    return Err(VMError::DecodeError);
                }
                words.push(item);
            }
            crate::list::allocate_words(vm, layout, &words)
        }
    }
}

/// Decode the record in `r11`, returning an aligned Blob word table in `r10`.
pub(crate) fn decode_state_value(vm: &mut IVM, resolver: AddressResolver) -> Result<u64, VMError> {
    let (schema, schema_payload) = decode_schema(vm, vm.register(10), resolver)?;
    let record_pointer = vm.register(11);
    if record_pointer == 0 {
        // Missing durable values are not valid typed values. StateMap.get/remove
        // branch on presence before typed decoding; top-level Scalar/aggregate
        // state must have been initialized by `hajimari`/`始まり`.
        return Err(VMError::DecodeError);
    }
    let (_, tlv) = load_expected_tlv(vm, record_pointer, PointerType::NoritoBytes, resolver)?;
    if tlv.payload.len() > MAX_STATE_VALUE_RECORD_BYTES {
        return Err(VMError::NoritoInvalid);
    }
    let record = decode_validated_state_value_record(
        vm.syscall_policy(),
        &schema,
        schema_payload,
        tlv.payload,
    )?;
    let atoms = record.atoms;
    let record_len = tlv.payload.len();

    let mut planned = Vec::new();
    let mut node_index = 0;
    let mut atom_index = 0;
    plan_state_atoms(
        &schema.nodes,
        &atoms,
        &mut node_index,
        &mut atom_index,
        &mut planned,
    )?;
    if node_index != schema.nodes.len()
        || atom_index != atoms.len()
        || planned.len() != schema.word_count().ok_or(VMError::DecodeError)?
    {
        return Err(VMError::DecodeError);
    }
    let pointer_bytes = planned
        .iter()
        .map(planned_state_bytes)
        .fold(0, usize::saturating_add);

    let table_payload_len = 1usize.saturating_add(planned.len().saturating_mul(8));
    let actual = gas(
        schema_payload
            .len()
            .saturating_add(record_len)
            .saturating_add(pointer_bytes)
            .saturating_add(table_payload_len),
        planned.len(),
    );
    preflight_reserved_syscall_gas(vm, actual)?;

    let mut tlv_lengths = Vec::new();
    let raw_heap_bytes = planned.iter().fold(0_usize, |bytes, value| {
        bytes.saturating_add(planned_state_allocation_shape(value, &mut tlv_lengths))
    });
    tlv_lengths.push(
        7_usize
            .saturating_add(table_payload_len)
            .saturating_add(Hash::LENGTH),
    );
    vm.preflight_host_tlv_allocations(&tlv_lengths)?;
    let raw_heap_bytes = u64::try_from(raw_heap_bytes).map_err(|_| VMError::OutOfMemory)?;
    if vm
        .memory
        .heap_allocated_len()
        .checked_add(raw_heap_bytes)
        .ok_or(VMError::OutOfMemory)?
        > vm.memory.heap_limit()
    {
        return Err(VMError::OutOfMemory);
    }

    let mut words = Vec::with_capacity(planned.len());
    for value in planned {
        words.push(materialize_state_word(vm, value)?);
    }
    let mut table = Vec::with_capacity(table_payload_len);
    table.push(0);
    for word in words {
        table.extend_from_slice(&word.to_le_bytes());
    }
    let out = encode_tlv(PointerType::Blob, &table)?;
    let pointer = vm.alloc_host_tlv(&out)?;
    vm.set_register(10, pointer);
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use iroha_primitives::{bigint::BigInt, numeric::Numeric};
    use ivm_abi::state_value::{
        StateValueAtomV1, StateValueNodeV1, StateValueRecordV1, StateValueSchemaV1,
    };

    use super::*;

    fn identity_address(_vm: &IVM, address: u64) -> u64 {
        address
    }

    fn install_schema(vm: &mut IVM, schema: &StateValueSchemaV1) -> u64 {
        let payload = to_bytes(schema).expect("schema bytes");
        let envelope = encode_tlv(PointerType::NoritoBytes, &payload).expect("schema TLV");
        vm.alloc_host_tlv(&envelope).expect("install schema")
    }

    fn install_int(vm: &mut IVM, value: i64) -> u64 {
        let frame = IntValueV1::try_new(BigInt::from_i128(i128::from(value)))
            .expect("test int is inside V1 domain")
            .encode_frame()
            .expect("canonical int frame");
        let envelope = encode_tlv(PointerType::Int, &frame).expect("int TLV");
        vm.alloc_host_tlv(&envelope).expect("install int")
    }

    fn install_quantity(vm: &mut IVM, value: &str) -> u64 {
        let quantity = value.parse().expect("canonical quantity");
        let frame = QuantityValueV1::new(quantity)
            .encode_frame()
            .expect("canonical quantity frame");
        let envelope = encode_tlv(PointerType::Quantity, &frame).expect("quantity TLV");
        vm.alloc_host_tlv(&envelope).expect("install quantity")
    }

    fn mixed_pointer_scalar_schema() -> StateValueSchemaV1 {
        StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Struct {
                    name: "Mixed".into(),
                    fields: vec!["label".into(), "enabled".into(), "count".into()],
                },
                StateValueNodeV1::Leaf(StateValueKindV1::Name),
                StateValueNodeV1::Leaf(StateValueKindV1::Bool),
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        }
    }

    fn assert_mixed_name_pointer_rejected<F>(install_pointer: F, expected: VMError)
    where
        F: FnOnce(&mut IVM) -> u64,
    {
        let schema = mixed_pointer_scalar_schema();
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &schema);
        let count_pointer = install_int(&mut vm, 7);
        let name_pointer = install_pointer(&mut vm);
        let table = vm.alloc_heap(24).expect("mixed aggregate word table");
        vm.store_u64(table, name_pointer)
            .expect("store candidate Name pointer");
        vm.store_u64(table + 8, 1).expect("store valid bool scalar");
        vm.store_u64(table + 16, count_pointer)
            .expect("store valid int pointer");
        let heap_before = vm.memory.heap_allocated_len();
        vm.set_register(10, schema_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 3);

        assert_eq!(encode_state_value(&mut vm, identity_address), Err(expected));
        assert_eq!(
            vm.register(10),
            schema_pointer,
            "a rejected input must not publish an output pointer"
        );
        assert_eq!(
            vm.memory.heap_allocated_len(),
            heap_before,
            "a rejected input must not allocate a partial output"
        );
    }

    #[test]
    fn schema_hash_is_domain_separated_and_stable() {
        let schema = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Int)],
        };
        let bytes = to_bytes(&schema).expect("schema bytes");
        assert_eq!(
            state_value_schema_hash_v1(&bytes),
            state_value_schema_hash_v1(&bytes)
        );
        assert_ne!(
            state_value_schema_hash_v1(&bytes),
            *Hash::new(&bytes).as_ref()
        );
    }

    #[test]
    fn aggregate_record_bytes_are_deterministic() {
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
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &schema);
        let table = vm.alloc_heap(16).expect("word table");
        let integer = install_int(&mut vm, 9);
        vm.store_u64(table, integer).expect("store integer pointer");
        vm.store_u64(table + 8, 1).expect("store boolean");

        let mut outputs = Vec::new();
        for _ in 0..2 {
            vm.set_register(10, schema_pointer);
            vm.set_register(11, table);
            vm.set_register(12, 2);
            encode_state_value(&mut vm, identity_address).expect("encode aggregate");
            let tlv = vm.validate_tlv(vm.register(10)).expect("encoded record");
            outputs.push(tlv.payload.to_vec());
        }
        assert_eq!(outputs[0], outputs[1]);
        let record: StateValueRecordV1 =
            decode_from_bytes(&outputs[0]).expect("decode stored record");
        assert!(matches!(
            record.atoms.as_slice(),
            [StateValueAtomV1::Pointer(_), StateValueAtomV1::Bool(true)]
        ));
    }

    #[test]
    fn decode_rejects_schema_mismatch() {
        let first = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Struct {
                    name: "First".into(),
                    fields: vec!["value".into()],
                },
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        };
        let second = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Struct {
                    name: "Second".into(),
                    fields: vec!["value".into()],
                },
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        };
        let mut vm = IVM::new(u64::MAX);
        let first_pointer = install_schema(&mut vm, &first);
        let table = vm.alloc_heap(8).expect("word table");
        let integer = install_int(&mut vm, 7);
        vm.store_u64(table, integer).expect("store integer pointer");
        vm.set_register(10, first_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 1);
        encode_state_value(&mut vm, identity_address).expect("encode aggregate");
        let record_pointer = vm.register(10);

        let second_pointer = install_schema(&mut vm, &second);
        vm.set_register(10, second_pointer);
        vm.set_register(11, record_pointer);
        assert!(matches!(
            decode_state_value(&mut vm, identity_address),
            Err(VMError::DecodeError)
        ));
    }

    #[test]
    fn decode_rejects_noncanonical_inactive_sum_payload() {
        let schema = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        };
        let schema_bytes = to_bytes(&schema).expect("schema bytes");
        let record = StateValueRecordV1 {
            schema_hash: state_value_schema_hash_v1(&schema_bytes),
            atoms: vec![
                StateValueAtomV1::Tag(false),
                StateValueAtomV1::Pointer(vec![99]),
            ],
        };
        let record = encode_tlv(
            PointerType::NoritoBytes,
            &to_bytes(&record).expect("record bytes"),
        )
        .expect("record TLV");
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &schema);
        let record_pointer = vm.alloc_host_tlv(&record).expect("install record");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, record_pointer);
        assert_eq!(
            decode_state_value(&mut vm, identity_address),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn missing_record_is_rejected_before_any_output_allocation() {
        let schema = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &schema);
        vm.set_register(10, schema_pointer);
        vm.set_register(11, 0);
        assert_eq!(
            decode_state_value(&mut vm, identity_address),
            Err(VMError::DecodeError)
        );
        assert_eq!(vm.register(10), schema_pointer);
    }

    #[test]
    fn encode_rejects_inactive_payload_words_and_null_active_pointers() {
        let option = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &option);
        let option_layout = SumLayoutV1::option(1).expect("Option layout");
        let forged = crate::sum::allocate_words(&mut vm, option_layout, 0, &[])
            .expect("Option::none to forge");
        vm.store_u64(forged + 8, 99)
            .expect("store hidden inactive payload");
        let table = vm.alloc_heap(8).expect("word table");
        vm.store_u64(table, forged).expect("store Option handle");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 1);
        assert_eq!(
            encode_state_value(&mut vm, identity_address),
            Err(VMError::DecodeError),
            "an inactive payload word is not part of the V1 value"
        );

        let none = crate::sum::allocate_words(&mut vm, option_layout, 0, &[])
            .expect("canonical Option::none");
        vm.store_u64(table, none).expect("store canonical Option");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 1);
        encode_state_value(&mut vm, identity_address).expect("encode active-only None");
        let encoded = vm.validate_tlv(vm.register(10)).expect("encoded record");
        let record: StateValueRecordV1 =
            decode_from_bytes(encoded.payload).expect("decode active-only record");
        assert_eq!(record.atoms, vec![StateValueAtomV1::Tag(false)]);

        let text = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::String)],
        };
        let schema_pointer = install_schema(&mut vm, &text);
        vm.store_u64(table, 0).expect("store null pointer");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 1);
        assert_eq!(
            encode_state_value(&mut vm, identity_address),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn typed_pointer_leaf_rejects_a_hash_valid_but_malformed_payload() {
        let schema = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Name)],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &schema);
        let forged = encode_tlv(PointerType::Name, b"not canonical Norito")
            .expect("hash-valid malformed Name TLV");
        let forged_pointer = vm.alloc_host_tlv(&forged).expect("install forged Name");
        let table = vm.alloc_heap(8).expect("word table");
        vm.store_u64(table, forged_pointer)
            .expect("store forged pointer");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 1);
        assert_eq!(
            encode_state_value(&mut vm, identity_address),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn mixed_pointer_scalar_record_rejects_missing_stale_and_malformed_dynamic_pointers() {
        assert_mixed_name_pointer_rejected(|_| 0, VMError::DecodeError);
        assert_mixed_name_pointer_rejected(|_| 1, VMError::NoritoInvalid);
        assert_mixed_name_pointer_rejected(|vm| install_int(vm, 7), VMError::NoritoInvalid);
        assert_mixed_name_pointer_rejected(
            |vm| {
                let envelope = encode_tlv(PointerType::Name, b"not canonical Norito")
                    .expect("hash-valid malformed Name TLV");
                vm.alloc_host_tlv(&envelope)
                    .expect("install malformed dynamic Name")
            },
            VMError::DecodeError,
        );
        assert_mixed_name_pointer_rejected(
            |vm| {
                let name: Name = "valid-name".parse().expect("valid Name");
                let mut envelope = encode_tlv(
                    PointerType::Name,
                    &to_bytes(&name).expect("encode canonical Name"),
                )
                .expect("valid Name TLV");
                let last = envelope.last_mut().expect("TLV checksum byte");
                *last ^= 1;
                vm.alloc_host_tlv(&envelope)
                    .expect("install checksum-corrupted Name")
            },
            VMError::NoritoInvalid,
        );
    }

    #[test]
    fn quantity_list_roundtrips_as_one_canonical_sequence_handle() {
        let element = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Quantity)],
        };
        let schema = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(element),
                capacity: 2,
            }],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &schema);
        let first_amount = install_quantity(&mut vm, "1.25");
        let second_amount = install_quantity(&mut vm, "1.25");
        let layout = ListLayoutV1::try_new(2, 1).expect("quantity list layout");
        let list_pointer = crate::list::allocate_words(
            &mut vm,
            layout,
            &[vec![first_amount], vec![second_amount]],
        )
        .expect("allocate contiguous quantity list");
        let table = vm.alloc_heap(8).expect("word table");
        vm.store_u64(table, list_pointer)
            .expect("store list pointer");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 1);
        encode_state_value(&mut vm, identity_address).expect("encode list state");
        let record_pointer = vm.register(10);

        vm.set_register(10, schema_pointer);
        vm.set_register(11, record_pointer);
        decode_state_value(&mut vm, identity_address).expect("decode list state");
        let table = vm.validate_tlv(vm.register(10)).expect("decoded table");
        let list_pointer = u64::from_le_bytes(table.payload[1..9].try_into().expect("list word"));
        let decoded =
            crate::list::read_words(&vm, list_pointer, layout).expect("read decoded list");
        assert_eq!(decoded.len(), 2);
        for item in &decoded {
            assert_eq!(item.len(), 1);
            let quantity = vm.validate_tlv(item[0]).expect("decoded quantity TLV");
            assert_eq!(quantity.type_id, PointerType::Quantity);
            let value = QuantityValueV1::decode_frame(quantity.payload)
                .expect("decode quantity")
                .into_quantity()
                .into_numeric();
            assert_eq!(value, Numeric::new(125, 2));
        }

        let overflow = crate::list::allocate_words(
            &mut vm,
            layout,
            &[vec![first_amount], vec![second_amount]],
        )
        .expect("allocate list to forge");
        vm.store_u64(overflow, 3)
            .expect("forge length past capacity");
        let overflow_table = vm.alloc_heap(8).expect("overflow word table");
        vm.store_u64(overflow_table, overflow)
            .expect("store overflow list pointer");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, overflow_table);
        vm.set_register(12, 1);
        assert_eq!(
            encode_state_value(&mut vm, identity_address),
            Err(VMError::DecodeError)
        );
    }

    #[test]
    fn list_of_nested_option_results_roundtrips_active_only_handles() {
        let element = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Result,
                StateValueNodeV1::Leaf(StateValueKindV1::Quantity),
                StateValueNodeV1::Leaf(StateValueKindV1::Bool),
            ],
        };
        let schema = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(element),
                capacity: 3,
            }],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &schema);
        let amount = install_quantity(&mut vm, "1.25");
        let result_layout = SumLayoutV1::try_new(1, 1).expect("Result layout");
        let option_layout = SumLayoutV1::option(1).expect("Option layout");
        let ok =
            crate::sum::allocate_words(&mut vm, result_layout, 1, &[amount]).expect("Result::ok");
        let some_ok =
            crate::sum::allocate_words(&mut vm, option_layout, 1, &[ok]).expect("Option::some ok");
        let err = crate::sum::allocate_words(&mut vm, result_layout, 0, &[1]).expect("Result::err");
        let some_err = crate::sum::allocate_words(&mut vm, option_layout, 1, &[err])
            .expect("Option::some err");
        let none =
            crate::sum::allocate_words(&mut vm, option_layout, 0, &[]).expect("Option::none");
        let list_layout = ListLayoutV1::try_new(3, 1).expect("list layout");
        let list = crate::list::allocate_words(
            &mut vm,
            list_layout,
            &[vec![some_ok], vec![some_err], vec![none]],
        )
        .expect("allocate list");
        let table = vm.alloc_heap(8).expect("word table");
        vm.store_u64(table, list).expect("store list handle");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 1);
        encode_state_value(&mut vm, identity_address).expect("encode nested sums");
        let record_pointer = vm.register(10);

        vm.set_register(10, schema_pointer);
        vm.set_register(11, record_pointer);
        decode_state_value(&mut vm, identity_address).expect("decode nested sums");
        let table = vm.validate_tlv(vm.register(10)).expect("decoded table");
        let list = u64::from_le_bytes(table.payload[1..9].try_into().expect("list word"));
        let list = crate::list::read_words(&vm, list, list_layout).expect("read list");
        assert_eq!(list.len(), 3);

        let (some, first) =
            crate::sum::read_words(&vm, list[0][0], option_layout).expect("read first Option");
        assert!(some);
        let (ok, amount) =
            crate::sum::read_words(&vm, first[0], result_layout).expect("read first Result");
        assert!(ok);
        let amount = vm.validate_tlv(amount[0]).expect("quantity TLV");
        let amount = QuantityValueV1::decode_frame(amount.payload)
            .expect("decode quantity")
            .into_quantity()
            .into_numeric();
        assert_eq!(amount, Numeric::new(125, 2));

        let (some, second) =
            crate::sum::read_words(&vm, list[1][0], option_layout).expect("read second Option");
        assert!(some);
        let (ok, error) =
            crate::sum::read_words(&vm, second[0], result_layout).expect("read second Result");
        assert!(!ok);
        assert_eq!(error, vec![1]);

        let (some, payload) =
            crate::sum::read_words(&vm, list[2][0], option_layout).expect("read Option::none");
        assert!(!some);
        assert!(payload.is_empty());
    }
}
