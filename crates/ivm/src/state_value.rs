//! Compiler-internal canonical codec for aggregate durable-state values.

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    prelude::{AssetDefinitionId, AssetId, DataSpaceId, DomainId, Name, NftId},
    soracloud::{SoracloudHostRequestEnvelopeV1, SoracloudHostResponseEnvelopeV1},
};
use iroha_primitives::{json::Json, numeric::Numeric};
use ivm_abi::state_value::{
    MAX_STATE_VALUE_RECORD_BYTES, MAX_STATE_VALUE_SCHEMA_BYTES, MAX_STATE_VALUE_WORDS,
    StateValueAtomV1, StateValueKindV1, StateValueRecordV1, StateValueSchemaV1,
    StateValueWordKindV1, state_value_schema_hash_v1,
};
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
    let address = resolver(vm, address);
    let tlv = vm.validate_tlv(address)?;
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
        return Err(VMError::NoritoInvalid);
    }
    Ok((envelope, tlv))
}

fn decode_schema<'a>(
    vm: &'a IVM,
    address: u64,
    resolver: AddressResolver,
) -> Result<(StateValueSchemaV1, &'a [u8]), VMError> {
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

fn pointer_type(kind: StateValueKindV1) -> Option<PointerType> {
    Some(match kind {
        StateValueKindV1::Int | StateValueKindV1::Bool => return None,
        StateValueKindV1::U128 | StateValueKindV1::Amount => PointerType::NoritoBytes,
        StateValueKindV1::String | StateValueKindV1::Bytes => PointerType::Blob,
        StateValueKindV1::Json => PointerType::Json,
        StateValueKindV1::AccountId => PointerType::AccountId,
        StateValueKindV1::AssetDefinitionId => PointerType::AssetDefinitionId,
        StateValueKindV1::AssetId => PointerType::AssetId,
        StateValueKindV1::DomainId => PointerType::DomainId,
        StateValueKindV1::NftId => PointerType::NftId,
        StateValueKindV1::Name => PointerType::Name,
        StateValueKindV1::DataSpaceId => PointerType::DataSpaceId,
        StateValueKindV1::AxtDescriptor => PointerType::AxtDescriptor,
        StateValueKindV1::AssetHandle => PointerType::AssetHandle,
        StateValueKindV1::ProofBlob => PointerType::ProofBlob,
        StateValueKindV1::SoracloudRequest => PointerType::SoracloudRequest,
        StateValueKindV1::SoracloudResponse => PointerType::SoracloudResponse,
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

fn validate_pointer_payload(kind: StateValueKindV1, payload: &[u8]) -> Result<(), VMError> {
    match kind {
        StateValueKindV1::Int | StateValueKindV1::Bool => return Err(VMError::DecodeError),
        StateValueKindV1::U128 => {
            let value: Numeric = decode_canonical_norito(payload)?;
            if value.scale() != 0 || value.try_mantissa_u128().is_none() {
                return Err(VMError::DecodeError);
            }
        }
        StateValueKindV1::Amount => {
            let _: Numeric = decode_canonical_norito(payload)?;
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
    if count > MAX_STATE_VALUE_WORDS || address % 8 != 0 {
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

/// Encode the compiler word table selected by `r11`/`r12` using the schema in `r10`.
pub(crate) fn encode_state_value(vm: &mut IVM, resolver: AddressResolver) -> Result<u64, VMError> {
    let (schema, schema_payload) = decode_schema(vm, vm.register(10), resolver)?;
    let word_kinds = schema.word_kinds().ok_or(VMError::DecodeError)?;
    let count = usize::try_from(vm.register(12)).map_err(|_| VMError::NoritoInvalid)?;
    if count != word_kinds.len() {
        return Err(VMError::DecodeError);
    }
    let words = table_words(vm, vm.register(11), count)?;
    let mut atoms = Vec::with_capacity(count);
    let mut pointer_bytes = 0_usize;
    for (kind, word) in word_kinds.into_iter().zip(words) {
        let atom = match kind {
            StateValueWordKindV1::Tag => match word {
                0 => StateValueAtomV1::Tag(false),
                1 => StateValueAtomV1::Tag(true),
                _ => return Err(VMError::DecodeError),
            },
            StateValueWordKindV1::Leaf(StateValueKindV1::Int) => StateValueAtomV1::Int(word as i64),
            StateValueWordKindV1::Leaf(StateValueKindV1::Bool) => match word {
                0 => StateValueAtomV1::Bool(false),
                1 => StateValueAtomV1::Bool(true),
                _ => return Err(VMError::DecodeError),
            },
            StateValueWordKindV1::Leaf(kind) => {
                let expected = pointer_type(kind).ok_or(VMError::DecodeError)?;
                if word == 0 {
                    StateValueAtomV1::Null
                } else {
                    let (envelope, tlv) = load_expected_tlv(vm, word, expected, resolver)?;
                    validate_pointer_payload(kind, tlv.payload)?;
                    pointer_bytes = pointer_bytes.saturating_add(envelope.len());
                    StateValueAtomV1::Pointer(envelope.to_vec())
                }
            }
        };
        atoms.push(atom);
    }
    if !schema.canonicalize_atoms(&mut atoms) {
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

/// Decode the record in `r11`, returning an aligned Blob word table in `r10`.
pub(crate) fn decode_state_value(vm: &mut IVM, resolver: AddressResolver) -> Result<u64, VMError> {
    let (schema, schema_payload) = decode_schema(vm, vm.register(10), resolver)?;
    let word_kinds = schema.word_kinds().ok_or(VMError::DecodeError)?;
    let record_pointer = vm.register(11);
    if record_pointer == 0 {
        // Missing durable values are not valid typed values. StateMap.get/remove
        // branch on presence and construct an internal inactive placeholder; top-level
        // Scalar/aggregate state must have been initialized by `hajimari`/`始まり`.
        return Err(VMError::DecodeError);
    }
    let (_, tlv) = load_expected_tlv(vm, record_pointer, PointerType::NoritoBytes, resolver)?;
    if tlv.payload.len() > MAX_STATE_VALUE_RECORD_BYTES {
        return Err(VMError::NoritoInvalid);
    }
    let record: StateValueRecordV1 =
        decode_from_bytes(tlv.payload).map_err(|_| VMError::DecodeError)?;
    if record.schema_hash != state_value_schema_hash_v1(schema_payload)
        || record.atoms.len() != word_kinds.len()
        || !schema.validate_atoms(&record.atoms)
        || to_bytes(&record)
            .map_err(|_| VMError::DecodeError)?
            .as_slice()
            != tlv.payload
    {
        return Err(VMError::DecodeError);
    }
    let atoms = record.atoms;
    let record_len = tlv.payload.len();

    let mut planned = Vec::with_capacity(atoms.len());
    let mut pointer_bytes = 0_usize;
    for (kind, atom) in word_kinds.iter().copied().zip(atoms) {
        match (kind, atom) {
            (StateValueWordKindV1::Tag, StateValueAtomV1::Tag(value)) => {
                planned.push(Ok(u64::from(value)))
            }
            (StateValueWordKindV1::Leaf(StateValueKindV1::Int), StateValueAtomV1::Int(value)) => {
                planned.push(Ok(value as u64))
            }
            (StateValueWordKindV1::Leaf(StateValueKindV1::Bool), StateValueAtomV1::Bool(value)) => {
                planned.push(Ok(u64::from(value)))
            }
            (StateValueWordKindV1::Leaf(kind), StateValueAtomV1::Null) if kind.is_pointer() => {
                planned.push(Ok(0))
            }
            (StateValueWordKindV1::Leaf(kind), StateValueAtomV1::Pointer(envelope))
                if kind.is_pointer() =>
            {
                let expected = pointer_type(kind).ok_or(VMError::DecodeError)?;
                let tlv = pointer_abi::validate_tlv_bytes(&envelope)?;
                if tlv.type_id != expected
                    || !pointer_abi::is_type_allowed_for_policy(vm.syscall_policy(), tlv.type_id)
                {
                    return Err(VMError::DecodeError);
                }
                if encode_tlv(tlv.type_id, tlv.payload)?.as_slice() != envelope {
                    return Err(VMError::DecodeError);
                }
                validate_pointer_payload(kind, tlv.payload)?;
                pointer_bytes = pointer_bytes.saturating_add(envelope.len());
                planned.push(Err(envelope));
            }
            _ => return Err(VMError::DecodeError),
        }
    }

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

    let mut words = Vec::with_capacity(planned.len());
    for value in planned {
        match value {
            Ok(word) => words.push(word),
            Err(envelope) => words.push(vm.alloc_host_tlv(&envelope)?),
        }
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
        vm.store_u64(table, 9).expect("store integer");
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
        assert_eq!(
            record.atoms,
            vec![StateValueAtomV1::Int(9), StateValueAtomV1::Bool(true)]
        );
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
        vm.store_u64(table, 7).expect("store integer");
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
            atoms: vec![StateValueAtomV1::Tag(false), StateValueAtomV1::Int(99)],
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
    fn encode_canonicalizes_inactive_payloads_and_rejects_null_active_pointers() {
        let option = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        };
        let mut vm = IVM::new(u64::MAX);
        let schema_pointer = install_schema(&mut vm, &option);
        let table = vm.alloc_heap(16).expect("word table");
        vm.store_u64(table, 0).expect("store absent tag");
        vm.store_u64(table + 8, 99).expect("store hidden payload");
        vm.set_register(10, schema_pointer);
        vm.set_register(11, table);
        vm.set_register(12, 2);
        encode_state_value(&mut vm, identity_address)
            .expect("inactive payload must be canonicalized");
        let encoded = vm.validate_tlv(vm.register(10)).expect("encoded record");
        let record: StateValueRecordV1 =
            decode_from_bytes(encoded.payload).expect("decode canonicalized record");
        assert_eq!(
            record.atoms,
            vec![StateValueAtomV1::Tag(false), StateValueAtomV1::Int(0)]
        );

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
}
