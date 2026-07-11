//! Exact, privacy-safe decoding of public Kotodama return registers.

use std::str;

use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    name::Name,
    nexus::DataSpaceId,
    nft::NftId,
    smart_contract::entrypoint::{
        ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1, EntrypointReturnRecordV1, EntrypointValueAtomV1,
        EntrypointValueKindV1, EntrypointValueTypeNodeV1, EntrypointValueTypeV1,
        MAX_ENTRYPOINT_RETURN_RECORD_BYTES, MAX_ENTRYPOINT_RETURN_WORDS,
        entrypoint_return_schema_hash_v1, entrypoint_value_subtree_range_v1,
    },
};
use iroha_primitives::{json::Json, numeric::Numeric};
use ivm::{IVM, PointerType, list::ListLayoutV1, sum::SumLayoutV1};
use norito::{
    codec::{Decode, Encode},
    json::{self, Map, Value},
};
use thiserror::Error;

const FIRST_RETURN_REGISTER: usize = 10;

/// Failure to decode the exact public return value declared by an entrypoint.
#[derive(Debug, Error)]
pub enum EntrypointReturnDecodeError {
    /// The signed schema is malformed or exceeds the V1 return register window.
    #[error("invalid entrypoint return schema")]
    InvalidSchema,
    /// A nested return record is not bound to the exact signed return schema.
    #[error("nested contract return record does not match its exact schema")]
    SchemaBinding,
    /// The canonical record exceeds the single-call V1 boundary.
    #[error(
        "entrypoint return record requires at least {bytes} encoded bytes; maximum is {max_bytes}"
    )]
    RecordTooLarge {
        /// Exact encoded size, or a guaranteed lower bound detected before cloning.
        bytes: usize,
        /// Active encoded-record limit (the V1 cap or a lower caller-affordability bound).
        max_bytes: usize,
    },
    /// Canonical Norito encoding or decoding failed inside the bounded envelope.
    #[error("invalid canonical entrypoint return record: {reason}")]
    RecordEncoding {
        /// Stable codec failure detail.
        reason: String,
    },
    /// A return word or pointed TLV crosses the ZK privacy boundary.
    #[error("contract return violates the ZK privacy boundary at r{register}: {reason}")]
    Privacy {
        /// Register which failed the public-boundary check.
        register: usize,
        /// Structured VM error rendered without exposing private data.
        reason: ivm::VMError,
    },
    /// An Option/Result tag or boolean is not the canonical scalar zero or one.
    #[error(
        "contract return at r{register} has non-canonical {role} value {value}; expected 0 or 1"
    )]
    NonCanonicalBit {
        /// Register containing the malformed bit.
        register: usize,
        /// Logical use of the bit.
        role: &'static str,
        /// Public malformed value.
        value: u64,
    },
    /// A public pointer has the wrong pointer-ABI type for the signed schema.
    #[error("contract return at r{register} has pointer type {actual:?}; expected {expected:?}")]
    PointerType {
        /// Register containing the pointer.
        register: usize,
        /// Expected pointer-ABI type.
        expected: PointerType,
        /// Actual pointer-ABI type.
        actual: PointerType,
    },
    /// A typed public TLV payload is invalid or non-canonical.
    #[error("invalid {kind} contract return at r{register}: {reason}")]
    InvalidValue {
        /// Register containing the value or pointer.
        register: usize,
        /// Human-readable signed schema kind.
        kind: &'static str,
        /// Stable failure detail.
        reason: String,
    },
}

struct ReturnRecordBudget {
    lower_bound_bytes: usize,
    max_bytes: usize,
}

impl Default for ReturnRecordBudget {
    fn default() -> Self {
        Self::new(MAX_ENTRYPOINT_RETURN_RECORD_BYTES)
    }
}

impl ReturnRecordBudget {
    fn new(max_bytes: usize) -> Self {
        Self {
            // Every framed record necessarily contains the Norito header and
            // its 32-byte schema-binding hash before any atom payload.
            lower_bound_bytes: norito::core::Header::SIZE + iroha_crypto::Hash::LENGTH,
            max_bytes: max_bytes.min(MAX_ENTRYPOINT_RETURN_RECORD_BYTES),
        }
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), EntrypointReturnDecodeError> {
        let next = self.lower_bound_bytes.checked_add(bytes).ok_or(
            EntrypointReturnDecodeError::RecordTooLarge {
                bytes: usize::MAX,
                max_bytes: self.max_bytes,
            },
        )?;
        if next > self.max_bytes {
            return Err(EntrypointReturnDecodeError::RecordTooLarge {
                bytes: next,
                max_bytes: self.max_bytes,
            });
        }
        self.lower_bound_bytes = next;
        Ok(())
    }

    fn reserve_atom(&mut self) -> Result<(), EntrypointReturnDecodeError> {
        // Every encoded enum atom consumes at least one byte.
        self.reserve(1)
    }

    fn reserve_list_atom(&mut self) -> Result<(), EntrypointReturnDecodeError> {
        // The flat V1 list tape owns an enum discriminant and one encoded `u8`
        // item count before its schema-delimited items.
        self.reserve(2)
    }

    fn reserve_pointer(
        &mut self,
        payload_bytes: usize,
    ) -> Result<usize, EntrypointReturnDecodeError> {
        let envelope_bytes = ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1
            .checked_add(payload_bytes)
            .ok_or(EntrypointReturnDecodeError::RecordTooLarge {
                bytes: usize::MAX,
                max_bytes: self.max_bytes,
            })?;
        // Charge the atom discriminant and the entire owned TLV before cloning.
        self.reserve(envelope_bytes.checked_add(1).ok_or(
            EntrypointReturnDecodeError::RecordTooLarge {
                bytes: usize::MAX,
                max_bytes: self.max_bytes,
            },
        )?)?;
        Ok(envelope_bytes)
    }
}

fn push_atom(
    atoms: &mut Vec<EntrypointValueAtomV1>,
    budget: &mut ReturnRecordBudget,
    atom: EntrypointValueAtomV1,
) -> Result<(), EntrypointReturnDecodeError> {
    budget.reserve_atom()?;
    atoms.push(atom);
    Ok(())
}

fn push_list_header(
    atoms: &mut Vec<EntrypointValueAtomV1>,
    budget: &mut ReturnRecordBudget,
    item_count: usize,
) -> Result<(), EntrypointReturnDecodeError> {
    let item_count =
        u8::try_from(item_count).map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?;
    budget.reserve_list_atom()?;
    atoms.push(EntrypointValueAtomV1::List(item_count));
    Ok(())
}

struct RegisterCursor<'vm, 'budget> {
    vm: &'vm IVM,
    register: usize,
    budget: &'budget mut ReturnRecordBudget,
}

impl RegisterCursor<'_, '_> {
    fn public_scalar(&mut self) -> Result<(usize, u64), EntrypointReturnDecodeError> {
        let register = self.register;
        self.vm
            .ensure_public_register(register)
            .map_err(|reason| EntrypointReturnDecodeError::Privacy { register, reason })?;
        let value = self.vm.register(register);
        self.register = self.register.saturating_add(1);
        Ok((register, value))
    }

    fn public_tlv(
        &mut self,
        kind: EntrypointValueKindV1,
    ) -> Result<(usize, Vec<u8>), EntrypointReturnDecodeError> {
        let register = self.register;
        self.vm
            .ensure_public_register(register)
            .map_err(|reason| EntrypointReturnDecodeError::Privacy { register, reason })?;
        let pointer = self.vm.register(register);
        let tlv = self
            .vm
            .validate_tlv(pointer)
            .map_err(|reason| EntrypointReturnDecodeError::Privacy { register, reason })?;
        let expected =
            expected_pointer_type(kind).ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
        if tlv.type_id != expected {
            return Err(EntrypointReturnDecodeError::PointerType {
                register,
                expected,
                actual: tlv.type_id,
            });
        }
        let expected_envelope_bytes = self.budget.reserve_pointer(tlv.payload.len())?;
        validate_pointer_payload(kind, tlv.payload, register)?;
        let envelope = self
            .vm
            .memory
            .load_region(
                pointer,
                u64::try_from(expected_envelope_bytes)
                    .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
            )
            .map_err(|error| handle_decode_error(register, kind_name(kind), error))?
            .to_vec();
        if envelope.len() != expected_envelope_bytes {
            return Err(EntrypointReturnDecodeError::InvalidValue {
                register,
                kind: kind_name(kind),
                reason: "validated TLV length changed while cloning".to_owned(),
            });
        }
        self.register = self.register.saturating_add(1);
        Ok((register, envelope))
    }
}

fn decode_canonical<T>(
    payload: &[u8],
    register: usize,
    kind: &'static str,
) -> Result<T, EntrypointReturnDecodeError>
where
    T: Decode + Encode,
{
    let value = norito::decode_from_bytes(payload).map_err(|error| {
        EntrypointReturnDecodeError::InvalidValue {
            register,
            kind,
            reason: error.to_string(),
        }
    })?;
    let canonical =
        norito::to_bytes(&value).map_err(|error| EntrypointReturnDecodeError::InvalidValue {
            register,
            kind,
            reason: error.to_string(),
        })?;
    if canonical != payload {
        return Err(EntrypointReturnDecodeError::InvalidValue {
            register,
            kind,
            reason: "non-canonical Norito payload".to_owned(),
        });
    }
    Ok(value)
}

fn expected_pointer_type(kind: EntrypointValueKindV1) -> Option<PointerType> {
    Some(match kind {
        EntrypointValueKindV1::Int | EntrypointValueKindV1::Bool => return None,
        EntrypointValueKindV1::U128 => PointerType::NoritoBytes,
        EntrypointValueKindV1::Amount => PointerType::Quantity,
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

fn kind_name(kind: EntrypointValueKindV1) -> &'static str {
    match kind {
        EntrypointValueKindV1::Int => "i64",
        EntrypointValueKindV1::U128 => "u128",
        EntrypointValueKindV1::Bool => "bool",
        EntrypointValueKindV1::String => "string",
        EntrypointValueKindV1::Amount => "Amount",
        EntrypointValueKindV1::Json => "Json",
        EntrypointValueKindV1::Name => "Name",
        EntrypointValueKindV1::AccountId => "AccountId",
        EntrypointValueKindV1::AssetDefinitionId => "AssetDefinitionId",
        EntrypointValueKindV1::AssetId => "AssetId",
        EntrypointValueKindV1::DomainId => "DomainId",
        EntrypointValueKindV1::NftId => "NftId",
        EntrypointValueKindV1::DataSpaceId => "DataSpaceId",
        EntrypointValueKindV1::Blob => "bytes",
    }
}

fn validate_pointer_payload(
    kind: EntrypointValueKindV1,
    payload: &[u8],
    register: usize,
) -> Result<(), EntrypointReturnDecodeError> {
    match kind {
        EntrypointValueKindV1::Int | EntrypointValueKindV1::Bool => {
            return Err(EntrypointReturnDecodeError::InvalidSchema);
        }
        EntrypointValueKindV1::U128 => {
            let value: Numeric = decode_canonical(payload, register, "u128")?;
            if value.scale() != 0 || value.try_mantissa_u128().is_none() {
                return Err(EntrypointReturnDecodeError::InvalidValue {
                    register,
                    kind: "u128",
                    reason: "expected a non-negative scale-zero Numeric fitting u128".to_owned(),
                });
            }
        }
        EntrypointValueKindV1::Amount => {
            let value: Numeric = decode_canonical(payload, register, "Amount")?;
            value
                .validate_amount()
                .map_err(|error| EntrypointReturnDecodeError::InvalidValue {
                    register,
                    kind: "Amount",
                    reason: error.to_string(),
                })?;
        }
        EntrypointValueKindV1::String => {
            str::from_utf8(payload).map_err(|error| EntrypointReturnDecodeError::InvalidValue {
                register,
                kind: "string",
                reason: error.to_string(),
            })?;
        }
        EntrypointValueKindV1::Json => {
            let _: Json = decode_canonical(payload, register, "Json")?;
        }
        EntrypointValueKindV1::Name => {
            let _: Name = decode_canonical(payload, register, "Name")?;
        }
        EntrypointValueKindV1::AccountId => {
            let _: AccountId = decode_canonical(payload, register, "AccountId")?;
        }
        EntrypointValueKindV1::AssetDefinitionId => {
            let _: AssetDefinitionId = decode_canonical(payload, register, "AssetDefinitionId")?;
        }
        EntrypointValueKindV1::AssetId => {
            let _: AssetId = decode_canonical(payload, register, "AssetId")?;
        }
        EntrypointValueKindV1::DomainId => {
            let _: DomainId = decode_canonical(payload, register, "DomainId")?;
        }
        EntrypointValueKindV1::NftId => {
            let _: NftId = decode_canonical(payload, register, "NftId")?;
        }
        EntrypointValueKindV1::DataSpaceId => {
            let _: DataSpaceId = decode_canonical(payload, register, "DataSpaceId")?;
        }
        EntrypointValueKindV1::Blob => {}
    }
    Ok(())
}

fn collect_leaf(
    cursor: &mut RegisterCursor<'_, '_>,
    kind: EntrypointValueKindV1,
    atoms: &mut Vec<EntrypointValueAtomV1>,
) -> Result<(), EntrypointReturnDecodeError> {
    match kind {
        EntrypointValueKindV1::Int => {
            let (_, value) = cursor.public_scalar()?;
            push_atom(
                atoms,
                cursor.budget,
                EntrypointValueAtomV1::Int(value as i64),
            )?;
        }
        EntrypointValueKindV1::Bool => {
            let (register, value) = cursor.public_scalar()?;
            let value = match value {
                0 => false,
                1 => true,
                value => {
                    return Err(EntrypointReturnDecodeError::NonCanonicalBit {
                        register,
                        role: "bool",
                        value,
                    });
                }
            };
            push_atom(atoms, cursor.budget, EntrypointValueAtomV1::Bool(value))?;
        }
        pointer_kind => {
            let (_, envelope) = cursor.public_tlv(pointer_kind)?;
            atoms.push(EntrypointValueAtomV1::Pointer(envelope));
        }
    }
    Ok(())
}

fn handle_decode_error(
    register: usize,
    kind: &'static str,
    error: ivm::VMError,
) -> EntrypointReturnDecodeError {
    if error == ivm::VMError::PrivacyViolation {
        EntrypointReturnDecodeError::Privacy {
            register,
            reason: error,
        }
    } else {
        EntrypointReturnDecodeError::InvalidValue {
            register,
            kind,
            reason: error.to_string(),
        }
    }
}

fn list_shape_error(register: usize, reason: impl Into<String>) -> EntrypointReturnDecodeError {
    EntrypointReturnDecodeError::InvalidValue {
        register,
        kind: "List",
        reason: reason.into(),
    }
}

fn return_node_child_count(node: &EntrypointValueTypeNodeV1) -> usize {
    match node {
        EntrypointValueTypeNodeV1::Struct(node) => node.fields.len(),
        EntrypointValueTypeNodeV1::Tuple(arity) => usize::from(*arity),
        EntrypointValueTypeNodeV1::Option | EntrypointValueTypeNodeV1::List(_) => 1,
        EntrypointValueTypeNodeV1::Result => 2,
        EntrypointValueTypeNodeV1::Leaf(_) => 0,
    }
}

/// Take exactly one checked preorder subtree and advance the shared cursor.
///
/// The schema has a hard V1 node bound, but this iterative walk also avoids
/// making native stack depth part of boundary validation. In particular, a
/// `List` owns the one element subtree immediately following its list node.
fn take_return_subtree<'a>(
    nodes: &'a [EntrypointValueTypeNodeV1],
    node_index: &mut usize,
) -> Result<&'a [EntrypointValueTypeNodeV1], EntrypointReturnDecodeError> {
    let start = *node_index;
    let range = entrypoint_value_subtree_range_v1(nodes, start)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    let end = range.end;
    let subtree = nodes
        .get(range)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    *node_index = end;
    Ok(subtree)
}

fn skip_return_node(
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
) -> Result<(), EntrypointReturnDecodeError> {
    let _ = take_return_subtree(nodes, node_index)?;
    Ok(())
}

fn return_child_starts(
    nodes: &[EntrypointValueTypeNodeV1],
    node_start: usize,
    child_count: usize,
) -> Result<Vec<usize>, EntrypointReturnDecodeError> {
    let root = entrypoint_value_subtree_range_v1(nodes, node_start)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    let mut child = node_start
        .checked_add(1)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    let mut starts = Vec::with_capacity(child_count);
    for _ in 0..child_count {
        starts.push(child);
        child = entrypoint_value_subtree_range_v1(nodes, child)
            .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
            .end;
    }
    if child != root.end {
        return Err(EntrypointReturnDecodeError::InvalidSchema);
    }
    Ok(starts)
}

fn return_node_word_count(
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
) -> Result<usize, EntrypointReturnDecodeError> {
    let subtree = take_return_subtree(nodes, node_index)?;
    let mut rendered = Vec::with_capacity(subtree.len());
    for node in subtree.iter().rev() {
        let child_count = return_node_child_count(node);
        if rendered.len() < child_count {
            return Err(EntrypointReturnDecodeError::InvalidSchema);
        }
        let children = rendered.split_off(rendered.len() - child_count);
        let words = match node {
            EntrypointValueTypeNodeV1::Struct(_) | EntrypointValueTypeNodeV1::Tuple(_) => children
                .into_iter()
                .try_fold(0_usize, usize::checked_add)
                .ok_or(EntrypointReturnDecodeError::InvalidSchema)?,
            EntrypointValueTypeNodeV1::Option
            | EntrypointValueTypeNodeV1::Result
            | EntrypointValueTypeNodeV1::List(_)
            | EntrypointValueTypeNodeV1::Leaf(_) => 1,
        };
        rendered.push(words);
    }
    (rendered.len() == 1)
        .then(|| rendered[0])
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)
}

fn collect_leaf_from_words(
    vm: &IVM,
    words: &[u64],
    word_index: &mut usize,
    register: usize,
    kind: EntrypointValueKindV1,
    atoms: &mut Vec<EntrypointValueAtomV1>,
    budget: &mut ReturnRecordBudget,
) -> Result<(), EntrypointReturnDecodeError> {
    let word = *words
        .get(*word_index)
        .ok_or_else(|| list_shape_error(register, "list item is missing an active word"))?;
    *word_index = word_index.saturating_add(1);
    match kind {
        EntrypointValueKindV1::Int => {
            push_atom(atoms, budget, EntrypointValueAtomV1::Int(word as i64))?;
        }
        EntrypointValueKindV1::Bool => {
            let value = match word {
                0 => false,
                1 => true,
                value => {
                    return Err(EntrypointReturnDecodeError::NonCanonicalBit {
                        register,
                        role: "list bool",
                        value,
                    });
                }
            };
            push_atom(atoms, budget, EntrypointValueAtomV1::Bool(value))?;
        }
        pointer_kind => {
            if word == 0 {
                return Err(list_shape_error(
                    register,
                    "list item contains a null typed pointer",
                ));
            }
            let expected = expected_pointer_type(pointer_kind)
                .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
            let tlv = vm
                .validate_tlv(word)
                .map_err(|error| handle_decode_error(register, "List", error))?;
            if tlv.type_id != expected {
                return Err(EntrypointReturnDecodeError::PointerType {
                    register,
                    expected,
                    actual: tlv.type_id,
                });
            }
            let expected_envelope_bytes = budget.reserve_pointer(tlv.payload.len())?;
            validate_pointer_payload(pointer_kind, tlv.payload, register)?;
            let envelope = vm
                .memory
                .load_region(
                    word,
                    u64::try_from(expected_envelope_bytes)
                        .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
                )
                .map_err(|error| handle_decode_error(register, "List", error))?
                .to_vec();
            if envelope.len() != expected_envelope_bytes {
                return Err(list_shape_error(
                    register,
                    "validated list-item TLV length changed while cloning",
                ));
            }
            atoms.push(EntrypointValueAtomV1::Pointer(envelope));
        }
    }
    Ok(())
}

fn collect_option_handle(
    vm: &IVM,
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
    register: usize,
    pointer: u64,
    atoms: &mut Vec<EntrypointValueAtomV1>,
    budget: &mut ReturnRecordBudget,
) -> Result<(), EntrypointReturnDecodeError> {
    if pointer == 0 {
        return Err(EntrypointReturnDecodeError::InvalidValue {
            register,
            kind: "Option",
            reason: "sum handle is null".to_owned(),
        });
    }
    let mut child_end = *node_index;
    let child_words = return_node_word_count(nodes, &mut child_end)?;
    let layout = SumLayoutV1::option(
        u64::try_from(child_words).map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
    )
    .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?;
    let (tag, payload) = ivm::sum::read_words(vm, pointer, layout)
        .map_err(|error| handle_decode_error(register, "Option", error))?;
    push_atom(atoms, budget, EntrypointValueAtomV1::Tag(tag))?;
    let mut active_word = 0;
    if tag {
        collect_node_from_words(
            vm,
            nodes,
            node_index,
            &payload,
            &mut active_word,
            register,
            atoms,
            budget,
        )?;
    } else {
        skip_return_node(nodes, node_index)?;
    }
    if *node_index != child_end || active_word != payload.len() {
        return Err(EntrypointReturnDecodeError::InvalidValue {
            register,
            kind: "Option",
            reason: "active payload does not match the selected branch schema".to_owned(),
        });
    }
    Ok(())
}

fn collect_result_handle(
    vm: &IVM,
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
    register: usize,
    pointer: u64,
    atoms: &mut Vec<EntrypointValueAtomV1>,
    budget: &mut ReturnRecordBudget,
) -> Result<(), EntrypointReturnDecodeError> {
    if pointer == 0 {
        return Err(EntrypointReturnDecodeError::InvalidValue {
            register,
            kind: "Result",
            reason: "sum handle is null".to_owned(),
        });
    }
    let mut ok_end = *node_index;
    let ok_words = return_node_word_count(nodes, &mut ok_end)?;
    let mut err_end = ok_end;
    let err_words = return_node_word_count(nodes, &mut err_end)?;
    let layout = SumLayoutV1::try_new(
        u64::try_from(err_words).map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
        u64::try_from(ok_words).map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
    )
    .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?;
    let (tag, payload) = ivm::sum::read_words(vm, pointer, layout)
        .map_err(|error| handle_decode_error(register, "Result", error))?;
    push_atom(atoms, budget, EntrypointValueAtomV1::Tag(tag))?;
    let mut active_word = 0;
    if tag {
        collect_node_from_words(
            vm,
            nodes,
            node_index,
            &payload,
            &mut active_word,
            register,
            atoms,
            budget,
        )?;
        skip_return_node(nodes, node_index)?;
    } else {
        skip_return_node(nodes, node_index)?;
        collect_node_from_words(
            vm,
            nodes,
            node_index,
            &payload,
            &mut active_word,
            register,
            atoms,
            budget,
        )?;
    }
    if *node_index != err_end || active_word != payload.len() {
        return Err(EntrypointReturnDecodeError::InvalidValue {
            register,
            kind: "Result",
            reason: "active payload does not match the selected branch schema".to_owned(),
        });
    }
    Ok(())
}

fn collect_node_from_words(
    vm: &IVM,
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
    words: &[u64],
    word_index: &mut usize,
    register: usize,
    atoms: &mut Vec<EntrypointValueAtomV1>,
    budget: &mut ReturnRecordBudget,
) -> Result<(), EntrypointReturnDecodeError> {
    enum WordInput<'words> {
        Borrowed { words: &'words [u64], index: usize },
        Owned { words: Vec<u64>, index: usize },
    }

    impl WordInput<'_> {
        fn next(&mut self) -> Option<u64> {
            let (words, index) = match self {
                Self::Borrowed { words, index } => (*words, index),
                Self::Owned { words, index } => (words.as_slice(), index),
            };
            let value = words.get(*index).copied()?;
            *index = index.saturating_add(1);
            Some(value)
        }

        fn is_exhausted(&self) -> bool {
            match self {
                Self::Borrowed { words, index } => *index == words.len(),
                Self::Owned { words, index } => *index == words.len(),
            }
        }

        fn position(&self) -> usize {
            match self {
                Self::Borrowed { index, .. } | Self::Owned { index, .. } => *index,
            }
        }
    }

    enum Task {
        Visit {
            node_start: usize,
            input: usize,
        },
        FinishActive {
            input: usize,
            kind: &'static str,
        },
        ContinueList {
            element_start: usize,
            remaining: std::vec::IntoIter<Vec<u64>>,
        },
        FinishListItem {
            element_start: usize,
            remaining: std::vec::IntoIter<Vec<u64>>,
            input: usize,
        },
    }

    fn next_word(
        inputs: &mut [WordInput<'_>],
        input: usize,
        register: usize,
        reason: &'static str,
    ) -> Result<u64, EntrypointReturnDecodeError> {
        inputs
            .get_mut(input)
            .and_then(WordInput::next)
            .ok_or_else(|| list_shape_error(register, reason))
    }

    fn subtree_words(
        nodes: &[EntrypointValueTypeNodeV1],
        start: usize,
    ) -> Result<(usize, usize), EntrypointReturnDecodeError> {
        let mut end = start;
        let words = return_node_word_count(nodes, &mut end)?;
        Ok((words, end))
    }

    fn own_words(inputs: &mut Vec<WordInput<'_>>, free: &mut Vec<usize>, words: Vec<u64>) -> usize {
        let input = free.pop().unwrap_or(inputs.len());
        let value = WordInput::Owned { words, index: 0 };
        if input == inputs.len() {
            inputs.push(value);
        } else {
            inputs[input] = value;
        }
        input
    }

    let root_start = *node_index;
    let root_end = entrypoint_value_subtree_range_v1(nodes, root_start)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
        .end;
    if *word_index > words.len() {
        return Err(EntrypointReturnDecodeError::InvalidSchema);
    }
    let mut inputs = vec![WordInput::Borrowed {
        words,
        index: *word_index,
    }];
    let mut free_inputs = Vec::<usize>::new();
    let mut tasks = vec![Task::Visit {
        node_start: root_start,
        input: 0,
    }];

    while let Some(task) = tasks.pop() {
        match task {
            Task::Visit { node_start, input } => match nodes
                .get(node_start)
                .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
            {
                EntrypointValueTypeNodeV1::Struct(node) => {
                    let starts = return_child_starts(nodes, node_start, node.fields.len())?;
                    tasks.extend(
                        starts
                            .into_iter()
                            .rev()
                            .map(|node_start| Task::Visit { node_start, input }),
                    );
                }
                EntrypointValueTypeNodeV1::Tuple(arity) => {
                    let starts = return_child_starts(nodes, node_start, usize::from(*arity))?;
                    tasks.extend(
                        starts
                            .into_iter()
                            .rev()
                            .map(|node_start| Task::Visit { node_start, input }),
                    );
                }
                EntrypointValueTypeNodeV1::Option => {
                    let pointer = next_word(
                        &mut inputs,
                        input,
                        register,
                        "list Option is missing its handle",
                    )?;
                    if pointer == 0 {
                        return Err(EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "Option",
                            reason: "sum handle is null".to_owned(),
                        });
                    }
                    let child_start = node_start
                        .checked_add(1)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    let (child_words, child_end) = subtree_words(nodes, child_start)?;
                    let expected_end = entrypoint_value_subtree_range_v1(nodes, node_start)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                        .end;
                    if child_end != expected_end {
                        return Err(EntrypointReturnDecodeError::InvalidSchema);
                    }
                    let layout = SumLayoutV1::option(
                        u64::try_from(child_words)
                            .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
                    )
                    .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?;
                    let (tag, payload) = ivm::sum::read_words(vm, pointer, layout)
                        .map_err(|error| handle_decode_error(register, "Option", error))?;
                    push_atom(atoms, budget, EntrypointValueAtomV1::Tag(tag))?;
                    if tag {
                        let active = own_words(&mut inputs, &mut free_inputs, payload);
                        tasks.push(Task::FinishActive {
                            input: active,
                            kind: "Option",
                        });
                        tasks.push(Task::Visit {
                            node_start: child_start,
                            input: active,
                        });
                    } else if !payload.is_empty() {
                        return Err(EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "Option",
                            reason: "active payload does not match the selected branch schema"
                                .to_owned(),
                        });
                    }
                }
                EntrypointValueTypeNodeV1::Result => {
                    let pointer = next_word(
                        &mut inputs,
                        input,
                        register,
                        "list Result is missing its handle",
                    )?;
                    if pointer == 0 {
                        return Err(EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "Result",
                            reason: "sum handle is null".to_owned(),
                        });
                    }
                    let starts = return_child_starts(nodes, node_start, 2)?;
                    let ok_start = starts[0];
                    let err_start = starts[1];
                    let (ok_words, ok_end) = subtree_words(nodes, ok_start)?;
                    let (err_words, err_end) = subtree_words(nodes, err_start)?;
                    let expected_end = entrypoint_value_subtree_range_v1(nodes, node_start)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                        .end;
                    if ok_end != err_start || err_end != expected_end {
                        return Err(EntrypointReturnDecodeError::InvalidSchema);
                    }
                    let layout = SumLayoutV1::try_new(
                        u64::try_from(err_words)
                            .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
                        u64::try_from(ok_words)
                            .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
                    )
                    .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?;
                    let (tag, payload) = ivm::sum::read_words(vm, pointer, layout)
                        .map_err(|error| handle_decode_error(register, "Result", error))?;
                    push_atom(atoms, budget, EntrypointValueAtomV1::Tag(tag))?;
                    let active = own_words(&mut inputs, &mut free_inputs, payload);
                    tasks.push(Task::FinishActive {
                        input: active,
                        kind: "Result",
                    });
                    tasks.push(Task::Visit {
                        node_start: if tag { ok_start } else { err_start },
                        input: active,
                    });
                }
                EntrypointValueTypeNodeV1::List(list) => {
                    let pointer = next_word(
                        &mut inputs,
                        input,
                        register,
                        "nested list is missing its handle",
                    )?;
                    if pointer == 0 {
                        return Err(list_shape_error(register, "list handle is null"));
                    }
                    let element_start = node_start
                        .checked_add(1)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    let (element_words, element_end) = subtree_words(nodes, element_start)?;
                    let expected_end = entrypoint_value_subtree_range_v1(nodes, node_start)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                        .end;
                    if element_end != expected_end {
                        return Err(EntrypointReturnDecodeError::InvalidSchema);
                    }
                    let layout = ListLayoutV1::try_new(
                        u64::from(list.capacity),
                        u64::try_from(element_words)
                            .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
                    )
                    .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?;
                    let raw_items = ivm::list::read_words(vm, pointer, layout)
                        .map_err(|error| handle_decode_error(register, "List", error))?;
                    push_list_header(atoms, budget, raw_items.len())?;
                    tasks.push(Task::ContinueList {
                        element_start,
                        remaining: raw_items.into_iter(),
                    });
                }
                EntrypointValueTypeNodeV1::Leaf(kind) => {
                    let words = match inputs
                        .get(input)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                    {
                        WordInput::Borrowed { words, .. } => *words,
                        WordInput::Owned { words, .. } => words.as_slice(),
                    };
                    let mut current = inputs
                        .get(input)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                        .position();
                    collect_leaf_from_words(
                        vm,
                        words,
                        &mut current,
                        register,
                        *kind,
                        atoms,
                        budget,
                    )?;
                    match inputs
                        .get_mut(input)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                    {
                        WordInput::Borrowed { index, .. } | WordInput::Owned { index, .. } => {
                            *index = current;
                        }
                    }
                }
            },
            Task::FinishActive { input, kind } => {
                if !inputs.get(input).is_some_and(WordInput::is_exhausted) {
                    return Err(EntrypointReturnDecodeError::InvalidValue {
                        register,
                        kind,
                        reason: "active payload does not match the selected branch schema"
                            .to_owned(),
                    });
                }
                inputs[input] = WordInput::Owned {
                    words: Vec::new(),
                    index: 0,
                };
                free_inputs.push(input);
            }
            Task::ContinueList {
                element_start,
                mut remaining,
            } => {
                if let Some(words) = remaining.next() {
                    let input = own_words(&mut inputs, &mut free_inputs, words);
                    tasks.push(Task::FinishListItem {
                        element_start,
                        remaining,
                        input,
                    });
                    tasks.push(Task::Visit {
                        node_start: element_start,
                        input,
                    });
                }
            }
            Task::FinishListItem {
                element_start,
                remaining,
                input,
            } => {
                if !inputs.get(input).is_some_and(WordInput::is_exhausted) {
                    return Err(list_shape_error(
                        register,
                        "list item does not have the exact flattened element width",
                    ));
                }
                inputs[input] = WordInput::Owned {
                    words: Vec::new(),
                    index: 0,
                };
                free_inputs.push(input);
                tasks.push(Task::ContinueList {
                    element_start,
                    remaining,
                });
            }
        }
    }

    *node_index = root_end;
    *word_index = inputs[0].position();
    Ok(())
}

fn collect_list_items(
    vm: &IVM,
    capacity: u8,
    element_nodes: &[EntrypointValueTypeNodeV1],
    register: usize,
    pointer: u64,
    atoms: &mut Vec<EntrypointValueAtomV1>,
    budget: &mut ReturnRecordBudget,
) -> Result<(), EntrypointReturnDecodeError> {
    if pointer == 0 {
        return Err(list_shape_error(register, "list handle is null"));
    }
    let mut element_end = 0;
    let element_words = return_node_word_count(element_nodes, &mut element_end)?;
    if element_end != element_nodes.len() {
        return Err(EntrypointReturnDecodeError::InvalidSchema);
    }
    let layout = ListLayoutV1::try_new(
        u64::from(capacity),
        u64::try_from(element_words).map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?,
    )
    .map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?;
    let raw_items = ivm::list::read_words(vm, pointer, layout)
        .map_err(|error| handle_decode_error(register, "List", error))?;
    push_list_header(atoms, budget, raw_items.len())?;
    for words in raw_items {
        let mut node_index = 0;
        let mut word_index = 0;
        collect_node_from_words(
            vm,
            element_nodes,
            &mut node_index,
            &words,
            &mut word_index,
            register,
            atoms,
            budget,
        )?;
        if node_index != element_nodes.len() || word_index != words.len() {
            return Err(list_shape_error(
                register,
                "list item does not have the exact flattened element width",
            ));
        }
    }
    Ok(())
}

fn collect_node(
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
    cursor: &mut RegisterCursor<'_, '_>,
    atoms: &mut Vec<EntrypointValueAtomV1>,
) -> Result<(), EntrypointReturnDecodeError> {
    let root_start = *node_index;
    let root_end = entrypoint_value_subtree_range_v1(nodes, root_start)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
        .end;
    let mut pending = vec![root_start];
    while let Some(node_start) = pending.pop() {
        match nodes
            .get(node_start)
            .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
        {
            EntrypointValueTypeNodeV1::Struct(node) => {
                let starts = return_child_starts(nodes, node_start, node.fields.len())?;
                pending.extend(starts.into_iter().rev());
            }
            EntrypointValueTypeNodeV1::Tuple(arity) => {
                let starts = return_child_starts(nodes, node_start, usize::from(*arity))?;
                pending.extend(starts.into_iter().rev());
            }
            EntrypointValueTypeNodeV1::Option => {
                let (register, pointer) = cursor.public_scalar()?;
                let mut child = node_start
                    .checked_add(1)
                    .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                collect_option_handle(
                    cursor.vm,
                    nodes,
                    &mut child,
                    register,
                    pointer,
                    atoms,
                    cursor.budget,
                )?;
                let expected_end = entrypoint_value_subtree_range_v1(nodes, node_start)
                    .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                    .end;
                if child != expected_end {
                    return Err(EntrypointReturnDecodeError::InvalidSchema);
                }
            }
            EntrypointValueTypeNodeV1::Result => {
                let (register, pointer) = cursor.public_scalar()?;
                let mut child = node_start
                    .checked_add(1)
                    .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                collect_result_handle(
                    cursor.vm,
                    nodes,
                    &mut child,
                    register,
                    pointer,
                    atoms,
                    cursor.budget,
                )?;
                let expected_end = entrypoint_value_subtree_range_v1(nodes, node_start)
                    .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                    .end;
                if child != expected_end {
                    return Err(EntrypointReturnDecodeError::InvalidSchema);
                }
            }
            EntrypointValueTypeNodeV1::List(list) => {
                let (register, pointer) = cursor.public_scalar()?;
                let mut element = node_start
                    .checked_add(1)
                    .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                let element_nodes = take_return_subtree(nodes, &mut element)?;
                let expected_end = entrypoint_value_subtree_range_v1(nodes, node_start)
                    .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                    .end;
                if element != expected_end {
                    return Err(EntrypointReturnDecodeError::InvalidSchema);
                }
                collect_list_items(
                    cursor.vm,
                    list.capacity,
                    element_nodes,
                    register,
                    pointer,
                    atoms,
                    cursor.budget,
                )?;
            }
            EntrypointValueTypeNodeV1::Leaf(kind) => collect_leaf(cursor, *kind, atoms)?,
        }
    }
    *node_index = root_end;
    Ok(())
}

fn schema_hash(schema: &EntrypointValueTypeV1) -> Result<[u8; 32], EntrypointReturnDecodeError> {
    let schema =
        norito::to_bytes(schema).map_err(|_| EntrypointReturnDecodeError::InvalidSchema)?;
    Ok(entrypoint_return_schema_hash_v1(&schema))
}

fn exact_record_bytes(
    record: &EntrypointReturnRecordV1,
    max_bytes: usize,
) -> Result<Vec<u8>, EntrypointReturnDecodeError> {
    let max_bytes = max_bytes.min(MAX_ENTRYPOINT_RETURN_RECORD_BYTES);
    // The bare length walk allocates no output buffer. It prevents an already
    // materialized adversarial record from forcing an oversized framed encode.
    let bare_bytes = record.encoded_len();
    if bare_bytes > max_bytes {
        return Err(EntrypointReturnDecodeError::RecordTooLarge {
            bytes: bare_bytes,
            max_bytes,
        });
    }
    let encoded =
        norito::to_bytes(record).map_err(|error| EntrypointReturnDecodeError::RecordEncoding {
            reason: error.to_string(),
        })?;
    if encoded.len() > max_bytes {
        return Err(EntrypointReturnDecodeError::RecordTooLarge {
            bytes: encoded.len(),
            max_bytes,
        });
    }
    Ok(encoded)
}

fn collect_entrypoint_return_record(
    vm: &IVM,
    schema: &EntrypointValueTypeV1,
    max_bytes: usize,
) -> Result<EntrypointReturnRecordV1, EntrypointReturnDecodeError> {
    let words = schema
        .word_count()
        .filter(|words| *words <= MAX_ENTRYPOINT_RETURN_WORDS)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    let mut budget = ReturnRecordBudget::new(max_bytes);
    let mut cursor = RegisterCursor {
        vm,
        register: FIRST_RETURN_REGISTER,
        budget: &mut budget,
    };
    let mut node_index = 0_usize;
    let mut atoms = Vec::with_capacity(words);
    collect_node(&schema.nodes, &mut node_index, &mut cursor, &mut atoms)?;
    let actual_words = cursor.register.saturating_sub(FIRST_RETURN_REGISTER);
    let actual_kinds = schema
        .word_kinds_for_atoms(&atoms)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    if node_index != schema.nodes.len()
        || actual_words != words
        || actual_kinds.len() != actual_words
    {
        return Err(EntrypointReturnDecodeError::InvalidSchema);
    }
    let record = EntrypointReturnRecordV1 {
        schema_hash: schema_hash(schema)?,
        atoms,
    };
    Ok(record)
}

/// Validate all public return registers and build the canonical typed record.
///
/// The full framed Norito length is validated even though this API returns the
/// structured record. Use [`encode_entrypoint_return_record_bytes`] when the
/// caller needs wire bytes and wants to avoid encoding the record twice.
///
/// # Errors
/// Returns an error for malformed schemas, private values, non-canonical
/// tags/booleans, malformed typed pointer payloads, or a record over 1 MiB.
pub fn encode_entrypoint_return_record(
    vm: &IVM,
    schema: &EntrypointValueTypeV1,
) -> Result<EntrypointReturnRecordV1, EntrypointReturnDecodeError> {
    let record = collect_entrypoint_return_record(vm, schema, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)?;
    let _ = exact_record_bytes(&record, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)?;
    Ok(record)
}

/// Validate public return registers and encode one bounded canonical record.
///
/// # Errors
/// Returns the same failures as [`encode_entrypoint_return_record`].
pub fn encode_entrypoint_return_record_bytes(
    vm: &IVM,
    schema: &EntrypointValueTypeV1,
) -> Result<Vec<u8>, EntrypointReturnDecodeError> {
    encode_entrypoint_return_record_bytes_bounded(vm, schema, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)
}

/// Validate public return registers and encode a canonical record without
/// cloning pointer payloads beyond `max_bytes`.
///
/// This is used by nested-call dispatch after converting the caller's gas
/// escrow into an affordable response-byte bound.
pub(crate) fn encode_entrypoint_return_record_bytes_bounded(
    vm: &IVM,
    schema: &EntrypointValueTypeV1,
    max_bytes: usize,
) -> Result<Vec<u8>, EntrypointReturnDecodeError> {
    let record = collect_entrypoint_return_record(vm, schema, max_bytes)?;
    exact_record_bytes(&record, max_bytes)
}

fn pointer_payload<'a>(
    atom: &'a EntrypointValueAtomV1,
    kind: EntrypointValueKindV1,
    register: usize,
) -> Result<&'a [u8], EntrypointReturnDecodeError> {
    let EntrypointValueAtomV1::Pointer(envelope) = atom else {
        return Err(EntrypointReturnDecodeError::InvalidValue {
            register,
            kind: kind_name(kind),
            reason: "expected a typed pointer atom".to_owned(),
        });
    };
    let tlv = ivm::pointer_abi::validate_tlv_bytes(envelope).map_err(|error| {
        EntrypointReturnDecodeError::InvalidValue {
            register,
            kind: kind_name(kind),
            reason: error.to_string(),
        }
    })?;
    let expected = expected_pointer_type(kind).ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    if tlv.type_id != expected {
        return Err(EntrypointReturnDecodeError::PointerType {
            register,
            expected,
            actual: tlv.type_id,
        });
    }
    validate_pointer_payload(kind, tlv.payload, register)?;
    Ok(tlv.payload)
}

fn render_leaf(
    atoms: &[EntrypointValueAtomV1],
    atom_index: &mut usize,
    kind: EntrypointValueKindV1,
) -> Result<Value, EntrypointReturnDecodeError> {
    let register = FIRST_RETURN_REGISTER.saturating_add(*atom_index);
    let atom = atoms
        .get(*atom_index)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    *atom_index = atom_index.saturating_add(1);
    match (kind, atom) {
        (EntrypointValueKindV1::Int, EntrypointValueAtomV1::Int(value)) => Ok(Value::from(*value)),
        (EntrypointValueKindV1::Bool, EntrypointValueAtomV1::Bool(value)) => {
            Ok(Value::Bool(*value))
        }
        (pointer_kind, EntrypointValueAtomV1::Pointer(_)) if pointer_kind.is_pointer() => {
            let payload = pointer_payload(atom, pointer_kind, register)?;
            Ok(match pointer_kind {
                EntrypointValueKindV1::U128 => {
                    let value: Numeric = decode_canonical(payload, register, "u128")?;
                    Value::from(
                        value
                            .try_mantissa_u128()
                            .filter(|_| value.scale() == 0)
                            .ok_or_else(|| EntrypointReturnDecodeError::InvalidValue {
                                register,
                                kind: "u128",
                                reason: "expected scale-zero u128".to_owned(),
                            })?
                            .to_string(),
                    )
                }
                EntrypointValueKindV1::Amount => {
                    let value: Numeric = decode_canonical(payload, register, "Amount")?;
                    value.validate_amount().map_err(|error| {
                        EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "Amount",
                            reason: error.to_string(),
                        }
                    })?;
                    Value::from(value.to_string())
                }
                EntrypointValueKindV1::String => Value::from(
                    str::from_utf8(payload)
                        .map_err(|error| EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "string",
                            reason: error.to_string(),
                        })?
                        .to_owned(),
                ),
                EntrypointValueKindV1::Json => {
                    let value: Json = decode_canonical(payload, register, "Json")?;
                    json::parse_value(value.get()).map_err(|error| {
                        EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "Json",
                            reason: error.to_string(),
                        }
                    })?
                }
                EntrypointValueKindV1::Name => {
                    Value::from(decode_canonical::<Name>(payload, register, "Name")?.to_string())
                }
                EntrypointValueKindV1::AccountId => Value::from(
                    decode_canonical::<AccountId>(payload, register, "AccountId")?.to_string(),
                ),
                EntrypointValueKindV1::AssetDefinitionId => Value::from(
                    decode_canonical::<AssetDefinitionId>(payload, register, "AssetDefinitionId")?
                        .to_string(),
                ),
                EntrypointValueKindV1::AssetId => Value::from(
                    decode_canonical::<AssetId>(payload, register, "AssetId")?.to_string(),
                ),
                EntrypointValueKindV1::DomainId => Value::from(
                    decode_canonical::<DomainId>(payload, register, "DomainId")?.to_string(),
                ),
                EntrypointValueKindV1::NftId => {
                    Value::from(decode_canonical::<NftId>(payload, register, "NftId")?.to_string())
                }
                EntrypointValueKindV1::DataSpaceId => Value::from(
                    decode_canonical::<DataSpaceId>(payload, register, "DataSpaceId")?.as_u64(),
                ),
                EntrypointValueKindV1::Blob => Value::from(format!("0x{}", hex::encode(payload))),
                EntrypointValueKindV1::Int | EntrypointValueKindV1::Bool => {
                    return Err(EntrypointReturnDecodeError::InvalidSchema);
                }
            })
        }
        _ => Err(EntrypointReturnDecodeError::InvalidValue {
            register,
            kind: kind_name(kind),
            reason: "atom kind does not match the exact schema".to_owned(),
        }),
    }
}

fn render_node(
    nodes: &[EntrypointValueTypeNodeV1],
    atoms: &[EntrypointValueAtomV1],
    node_index: &mut usize,
    atom_index: &mut usize,
) -> Result<Value, EntrypointReturnDecodeError> {
    enum ProductKind<'a> {
        Struct(&'a [String]),
        Tuple,
    }

    enum Continuation<'a> {
        Product {
            kind: ProductKind<'a>,
            child_starts: Vec<usize>,
            next_child: usize,
            values: Vec<Value>,
        },
        Wrap {
            key: &'static str,
        },
        List {
            element_start: usize,
            next_item: usize,
            item_count: usize,
            values: Vec<Value>,
        },
    }

    #[derive(Clone, Copy)]
    struct Visit {
        node_start: usize,
        atom_start: usize,
    }

    let root_start = *node_index;
    let root_end = entrypoint_value_subtree_range_v1(nodes, root_start)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
        .end;
    let mut current = Some(Visit {
        node_start: root_start,
        atom_start: *atom_index,
    });
    let mut continuations = Vec::<Continuation<'_>>::new();
    let mut completed = None::<(Value, usize)>;

    loop {
        if let Some(visit) = current.take() {
            let node = nodes
                .get(visit.node_start)
                .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
            match node {
                EntrypointValueTypeNodeV1::Struct(node) => {
                    let starts = return_child_starts(nodes, visit.node_start, node.fields.len())?;
                    let Some(first) = starts.first().copied() else {
                        completed = Some((Value::Object(Map::new()), visit.atom_start));
                        continue;
                    };
                    continuations.push(Continuation::Product {
                        kind: ProductKind::Struct(&node.fields),
                        child_starts: starts,
                        next_child: 1,
                        values: Vec::with_capacity(node.fields.len()),
                    });
                    current = Some(Visit {
                        node_start: first,
                        atom_start: visit.atom_start,
                    });
                }
                EntrypointValueTypeNodeV1::Tuple(arity) => {
                    let count = usize::from(*arity);
                    let starts = return_child_starts(nodes, visit.node_start, count)?;
                    let Some(first) = starts.first().copied() else {
                        completed = Some((Value::Array(Vec::new()), visit.atom_start));
                        continue;
                    };
                    continuations.push(Continuation::Product {
                        kind: ProductKind::Tuple,
                        child_starts: starts,
                        next_child: 1,
                        values: Vec::with_capacity(count),
                    });
                    current = Some(Visit {
                        node_start: first,
                        atom_start: visit.atom_start,
                    });
                }
                EntrypointValueTypeNodeV1::Option => {
                    let register = FIRST_RETURN_REGISTER.saturating_add(visit.atom_start);
                    let Some(EntrypointValueAtomV1::Tag(tag)) = atoms.get(visit.atom_start) else {
                        return Err(EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "Option",
                            reason: "expected a canonical tag atom".to_owned(),
                        });
                    };
                    let next_atom = visit
                        .atom_start
                        .checked_add(1)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    if *tag {
                        continuations.push(Continuation::Wrap { key: "some" });
                        current = Some(Visit {
                            node_start: visit
                                .node_start
                                .checked_add(1)
                                .ok_or(EntrypointReturnDecodeError::InvalidSchema)?,
                            atom_start: next_atom,
                        });
                    } else {
                        completed = Some((
                            Value::Object(Map::from_iter([("none".to_owned(), Value::Bool(true))])),
                            next_atom,
                        ));
                    }
                }
                EntrypointValueTypeNodeV1::Result => {
                    let register = FIRST_RETURN_REGISTER.saturating_add(visit.atom_start);
                    let Some(EntrypointValueAtomV1::Tag(tag)) = atoms.get(visit.atom_start) else {
                        return Err(EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "Result",
                            reason: "expected a canonical tag atom".to_owned(),
                        });
                    };
                    let ok_start = visit
                        .node_start
                        .checked_add(1)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    let err_start = entrypoint_value_subtree_range_v1(nodes, ok_start)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?
                        .end;
                    continuations.push(Continuation::Wrap {
                        key: if *tag { "ok" } else { "err" },
                    });
                    current = Some(Visit {
                        node_start: if *tag { ok_start } else { err_start },
                        atom_start: visit
                            .atom_start
                            .checked_add(1)
                            .ok_or(EntrypointReturnDecodeError::InvalidSchema)?,
                    });
                }
                EntrypointValueTypeNodeV1::List(list) => {
                    let register = FIRST_RETURN_REGISTER.saturating_add(visit.atom_start);
                    let Some(EntrypointValueAtomV1::List(item_count)) = atoms.get(visit.atom_start)
                    else {
                        return Err(EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "List",
                            reason: "expected a canonical list atom".to_owned(),
                        });
                    };
                    let item_count = usize::from(*item_count);
                    if item_count > usize::from(list.capacity) {
                        return Err(EntrypointReturnDecodeError::InvalidValue {
                            register,
                            kind: "List",
                            reason: "list payload exceeds its schema capacity".to_owned(),
                        });
                    }
                    let element_start = visit
                        .node_start
                        .checked_add(1)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    let _ = entrypoint_value_subtree_range_v1(nodes, element_start)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    let first_item_atom = visit
                        .atom_start
                        .checked_add(1)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    if item_count == 0 {
                        completed = Some((Value::Array(Vec::new()), first_item_atom));
                        continue;
                    }
                    continuations.push(Continuation::List {
                        element_start,
                        next_item: 1,
                        item_count,
                        values: Vec::with_capacity(item_count),
                    });
                    current = Some(Visit {
                        node_start: element_start,
                        atom_start: first_item_atom,
                    });
                }
                EntrypointValueTypeNodeV1::Leaf(kind) => {
                    let mut next_atom = visit.atom_start;
                    let value = render_leaf(atoms, &mut next_atom, *kind)?;
                    completed = Some((value, next_atom));
                }
            }
            continue;
        }

        let (value, child_atom_end) = completed
            .take()
            .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
        let Some(continuation) = continuations.pop() else {
            *node_index = root_end;
            *atom_index = child_atom_end;
            return Ok(value);
        };
        match continuation {
            Continuation::Product {
                kind,
                child_starts,
                mut next_child,
                mut values,
            } => {
                values.push(value);
                if let Some(next_start) = child_starts.get(next_child).copied() {
                    next_child = next_child
                        .checked_add(1)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    continuations.push(Continuation::Product {
                        kind,
                        child_starts,
                        next_child,
                        values,
                    });
                    current = Some(Visit {
                        node_start: next_start,
                        atom_start: child_atom_end,
                    });
                } else {
                    completed = Some((
                        match kind {
                            ProductKind::Struct(fields) => {
                                if fields.len() != values.len() {
                                    return Err(EntrypointReturnDecodeError::InvalidSchema);
                                }
                                Value::Object(Map::from_iter(
                                    fields.iter().cloned().zip(values.into_iter()),
                                ))
                            }
                            ProductKind::Tuple => Value::Array(values),
                        },
                        child_atom_end,
                    ));
                }
            }
            Continuation::Wrap { key } => {
                completed = Some((
                    Value::Object(Map::from_iter([(key.to_owned(), value)])),
                    child_atom_end,
                ));
            }
            Continuation::List {
                element_start,
                mut next_item,
                item_count,
                mut values,
            } => {
                values.push(value);
                if next_item < item_count {
                    next_item = next_item
                        .checked_add(1)
                        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
                    continuations.push(Continuation::List {
                        element_start,
                        next_item,
                        item_count,
                        values,
                    });
                    current = Some(Visit {
                        node_start: element_start,
                        atom_start: child_atom_end,
                    });
                } else {
                    completed = Some((Value::Array(values), child_atom_end));
                }
            }
        }
    }
}

fn render_entrypoint_return_record_validated(
    schema: &EntrypointValueTypeV1,
    record: &EntrypointReturnRecordV1,
) -> Result<Value, EntrypointReturnDecodeError> {
    let words = schema
        .word_count()
        .filter(|words| *words <= MAX_ENTRYPOINT_RETURN_WORDS)
        .ok_or(EntrypointReturnDecodeError::InvalidSchema)?;
    let actual_words = schema
        .word_kinds_for_atoms(&record.atoms)
        .ok_or(EntrypointReturnDecodeError::SchemaBinding)?;
    if actual_words.len() != words {
        return Err(EntrypointReturnDecodeError::SchemaBinding);
    }
    if record.schema_hash != schema_hash(schema)? {
        return Err(EntrypointReturnDecodeError::SchemaBinding);
    }
    let mut node_index = 0_usize;
    let mut atom_index = 0_usize;
    let value = render_node(
        &schema.nodes,
        &record.atoms,
        &mut node_index,
        &mut atom_index,
    )?;
    if node_index != schema.nodes.len() || atom_index != record.atoms.len() {
        return Err(EntrypointReturnDecodeError::InvalidSchema);
    }
    Ok(value)
}

/// Render a canonical nested return record for a client-facing JSON boundary.
///
/// # Errors
/// Returns an error when the record is oversized, not schema-bound, or atom
/// kinds differ.
pub fn render_entrypoint_return_record(
    schema: &EntrypointValueTypeV1,
    record: &EntrypointReturnRecordV1,
) -> Result<Value, EntrypointReturnDecodeError> {
    let _ = exact_record_bytes(record, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)?;
    render_entrypoint_return_record_validated(schema, record)
}

/// Decode and validate one canonical schema-bound nested return record.
///
/// The byte limit is enforced before Norito decoding. The decoded value is
/// re-encoded byte-for-byte so trailing data, alternate layouts, and malformed
/// inactive branches fail closed before any client-facing rendering.
///
/// # Errors
/// Returns an error for oversized or non-canonical bytes, schema mismatch, or
/// invalid typed atoms.
pub fn decode_entrypoint_return_record(
    schema: &EntrypointValueTypeV1,
    payload: &[u8],
) -> Result<EntrypointReturnRecordV1, EntrypointReturnDecodeError> {
    if payload.len() > MAX_ENTRYPOINT_RETURN_RECORD_BYTES {
        return Err(EntrypointReturnDecodeError::RecordTooLarge {
            bytes: payload.len(),
            max_bytes: MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
        });
    }
    let record: EntrypointReturnRecordV1 = norito::decode_from_bytes(payload).map_err(|error| {
        EntrypointReturnDecodeError::RecordEncoding {
            reason: error.to_string(),
        }
    })?;
    if exact_record_bytes(&record, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)?.as_slice() != payload {
        return Err(EntrypointReturnDecodeError::RecordEncoding {
            reason: "record is not the byte-exact canonical Norito encoding".to_owned(),
        });
    }
    let _ = render_entrypoint_return_record_validated(schema, &record)?;
    Ok(record)
}

/// Decode a non-unit return value from `r10..r22` for Torii/CLI JSON output.
///
/// Only the active `Option`/`Result` branch is read. Runtime-to-runtime calls should use
/// [`encode_entrypoint_return_record`] and keep the wire representation typed.
///
/// # Errors
/// Returns an error for malformed schemas, non-canonical words, private
/// values/TLVs, schema-binding failures, or typed decode failures.
pub fn decode_entrypoint_return(
    vm: &IVM,
    schema: &EntrypointValueTypeV1,
) -> Result<Value, EntrypointReturnDecodeError> {
    let record = collect_entrypoint_return_record(vm, schema, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)?;
    let _ = exact_record_bytes(&record, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)?;
    render_entrypoint_return_record_validated(schema, &record)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointListTypeNodeV1, EntrypointStructTypeNodeV1, MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH,
    };

    use super::*;

    fn leaf(kind: EntrypointValueKindV1) -> EntrypointValueTypeV1 {
        EntrypointValueTypeV1 {
            nodes: vec![EntrypointValueTypeNodeV1::Leaf(kind)],
        }
    }

    fn list(capacity: u8, element: EntrypointValueTypeV1) -> EntrypointValueTypeV1 {
        let mut nodes = Vec::with_capacity(1 + element.nodes.len());
        nodes.push(EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
            capacity,
        }));
        nodes.extend(element.nodes);
        EntrypointValueTypeV1 { nodes }
    }

    fn nested_list_schema(levels: usize) -> EntrypointValueTypeV1 {
        let mut nodes = Vec::with_capacity(levels.saturating_add(1));
        for _ in 0..levels {
            nodes.push(EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
                capacity: 1,
            }));
        }
        nodes.push(EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int));
        EntrypointValueTypeV1 { nodes }
    }

    fn nested_tuple_schema(levels: usize) -> EntrypointValueTypeV1 {
        let mut nodes = Vec::with_capacity(levels.saturating_add(1));
        nodes.extend((0..levels).map(|_| EntrypointValueTypeNodeV1::Tuple(1)));
        nodes.push(EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int));
        EntrypointValueTypeV1 { nodes }
    }

    fn nested_option_schema(levels: usize) -> EntrypointValueTypeV1 {
        let mut nodes = Vec::with_capacity(levels.saturating_add(1));
        nodes.extend((0..levels).map(|_| EntrypointValueTypeNodeV1::Option));
        nodes.push(EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int));
        EntrypointValueTypeV1 { nodes }
    }

    fn test_tlv(ty: PointerType, payload: &[u8]) -> Vec<u8> {
        let mut envelope = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
        envelope.extend_from_slice(&(ty as u16).to_be_bytes());
        envelope.push(1);
        envelope.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("test payload fits u32")
                .to_be_bytes(),
        );
        envelope.extend_from_slice(payload);
        envelope.extend_from_slice(Hash::new(payload).as_ref());
        envelope
    }

    fn input_tlv(vm: &mut IVM, ty: PointerType, payload: &[u8]) -> u64 {
        let envelope = test_tlv(ty, payload);
        vm.alloc_input_tlv(&envelope).expect("allocate test TLV")
    }

    fn nested_schema() -> EntrypointValueTypeV1 {
        EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "Receipt".to_owned(),
                    fields: vec!["maybe".to_owned(), "outcome".to_owned(), "label".to_owned()],
                }),
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
                EntrypointValueTypeNodeV1::Result,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
                EntrypointValueTypeNodeV1::Tuple(2),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
            ],
        }
    }

    #[test]
    fn flat_list_element_cursor_consumes_exactly_one_bounded_subtree() {
        let nodes = vec![
            EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 4 }),
            EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                name: "Pair".to_owned(),
                fields: vec!["left".to_owned(), "right".to_owned()],
            }),
            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            EntrypointValueTypeNodeV1::Option,
            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Amount),
        ];
        let mut list_element = 1;
        let subtree = take_return_subtree(&nodes, &mut list_element)
            .expect("one exact element subtree follows the List node");
        assert_eq!(subtree, &nodes[1..]);
        assert_eq!(list_element, nodes.len());

        let mut list_root = 0;
        assert_eq!(
            return_node_word_count(&nodes, &mut list_root).expect("valid flat List schema"),
            1
        );
        assert_eq!(list_root, nodes.len());

        let mut malformed = 1;
        assert!(matches!(
            take_return_subtree(&nodes[..4], &mut malformed),
            Err(EntrypointReturnDecodeError::InvalidSchema)
        ));
        assert_eq!(malformed, 1, "a rejected cursor must not advance");
    }

    #[test]
    fn maximum_flat_list_depth_renders_without_native_stack_recursion() {
        let levels = MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1;
        let schema = nested_list_schema(levels);
        assert!(schema.validate());

        let mut atoms = Vec::with_capacity(levels.saturating_add(1));
        atoms.extend((0..levels).map(|_| EntrypointValueAtomV1::List(1)));
        atoms.push(EntrypointValueAtomV1::Int(7));
        let record = EntrypointReturnRecordV1 {
            schema_hash: schema_hash(&schema).expect("hash the exact boundary schema"),
            atoms,
        };
        let rendered = render_entrypoint_return_record(&schema, &record)
            .expect("render the exact V1 nesting boundary");
        let mut cursor = &rendered;
        for _ in 0..levels {
            let items = cursor.as_array().expect("nested list renders as an array");
            assert_eq!(items.len(), 1);
            cursor = &items[0];
        }
        assert_eq!(cursor, &Value::from(7));

        let over_limit = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH);
        assert!(!over_limit.validate());
        assert!(matches!(
            render_entrypoint_return_record(&over_limit, &record),
            Err(EntrypointReturnDecodeError::InvalidSchema)
                | Err(EntrypointReturnDecodeError::SchemaBinding)
        ));
    }

    #[test]
    fn maximum_depth_return_collectors_use_bounded_work_stacks() {
        let levels = MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1;
        let mut vm = IVM::new(10_000);

        let tuple_schema = nested_tuple_schema(levels);
        assert!(tuple_schema.validate());
        vm.set_register(FIRST_RETURN_REGISTER, 7);
        let tuple_record = collect_entrypoint_return_record(
            &vm,
            &tuple_schema,
            MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
        )
        .expect("collect the maximum-depth product without native recursion");
        assert_eq!(tuple_record.atoms, vec![EntrypointValueAtomV1::Int(7)]);

        let list_schema = nested_list_schema(levels);
        assert!(list_schema.validate());
        let list_layout = ListLayoutV1::try_new(1, 1).expect("one-word list layout");
        let mut list_word = 9_u64;
        for _ in 0..levels {
            list_word = ivm::list::allocate_words(&mut vm, list_layout, &[vec![list_word]])
                .expect("allocate one nested active list item");
        }
        vm.set_register(FIRST_RETURN_REGISTER, list_word);
        let list_record =
            collect_entrypoint_return_record(&vm, &list_schema, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)
                .expect("collect the maximum-depth flat List tape without native recursion");
        assert_eq!(list_record.atoms.len(), levels + 1);
        assert!(
            list_record.atoms[..levels]
                .iter()
                .all(|atom| atom == &EntrypointValueAtomV1::List(1))
        );
        assert_eq!(
            list_record.atoms.last(),
            Some(&EntrypointValueAtomV1::Int(9))
        );

        let option_schema = nested_option_schema(levels);
        assert!(option_schema.validate());
        let option_layout = SumLayoutV1::option(1).expect("one-word Option layout");
        let mut option_word = 11_u64;
        for _ in 0..levels {
            option_word = ivm::sum::allocate_words(&mut vm, option_layout, 1, &[option_word])
                .expect("allocate one nested active Option payload");
        }
        vm.set_register(FIRST_RETURN_REGISTER, option_word);
        let option_record = collect_entrypoint_return_record(
            &vm,
            &option_schema,
            MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
        )
        .expect("collect the maximum-depth active Option chain without native recursion");
        assert_eq!(option_record.atoms.len(), levels + 1);
        assert!(
            option_record.atoms[..levels]
                .iter()
                .all(|atom| atom == &EntrypointValueAtomV1::Tag(true))
        );
        assert_eq!(
            option_record.atoms.last(),
            Some(&EntrypointValueAtomV1::Int(11))
        );
    }

    #[test]
    fn maximum_depth_active_payload_rejects_a_null_nested_sum_handle() {
        let levels = MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1;
        let schema = nested_option_schema(levels);
        assert!(schema.validate());
        let mut vm = IVM::new(10_000);
        let layout = SumLayoutV1::option(1).expect("one-word Option layout");
        let mut pointer = 0_u64;
        for _ in 0..levels.saturating_sub(1) {
            pointer = ivm::sum::allocate_words(&mut vm, layout, 1, &[pointer])
                .expect("wrap the malformed active child in a valid outer handle");
        }
        vm.set_register(FIRST_RETURN_REGISTER, pointer);

        assert!(matches!(
            collect_entrypoint_return_record(
                &vm,
                &schema,
                MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
            ),
            Err(EntrypointReturnDecodeError::InvalidValue {
                kind: "Option",
                reason,
                ..
            }) if reason == "sum handle is null"
        ));
    }

    #[test]
    fn nested_struct_option_and_result_render_exact_json() {
        let schema = nested_schema();
        let mut vm = IVM::new(10_000);
        let label = input_tlv(&mut vm, PointerType::Blob, "言挙げ".as_bytes());
        let maybe = ivm::sum::allocate_words(
            &mut vm,
            SumLayoutV1::option(1).expect("Option layout"),
            0,
            &[],
        )
        .expect("Option::none");
        let outcome = ivm::sum::allocate_words(
            &mut vm,
            SumLayoutV1::try_new(2, 1).expect("Result layout"),
            1,
            &[1],
        )
        .expect("Result::ok");
        for (offset, value) in [maybe, outcome, label].into_iter().enumerate() {
            vm.set_register(FIRST_RETURN_REGISTER + offset, value);
        }
        let value = decode_entrypoint_return(&vm, &schema).expect("decode exact nested return");
        assert_eq!(
            value,
            norito::json!({
                "maybe": { "none": true },
                "outcome": { "ok": true },
                "label": "言挙げ",
            })
        );
    }

    #[test]
    fn typed_return_record_roundtrips_and_binds_the_exact_schema() {
        let schema = nested_schema();
        let mut vm = IVM::new(10_000);
        let label = input_tlv(&mut vm, PointerType::Blob, b"label");
        let maybe = ivm::sum::allocate_words(
            &mut vm,
            SumLayoutV1::option(1).expect("Option layout"),
            0,
            &[],
        )
        .expect("Option::none");
        let outcome = ivm::sum::allocate_words(
            &mut vm,
            SumLayoutV1::try_new(2, 1).expect("Result layout"),
            1,
            &[1],
        )
        .expect("Result::ok");
        for (offset, value) in [maybe, outcome, label].into_iter().enumerate() {
            vm.set_register(FIRST_RETURN_REGISTER + offset, value);
        }
        let record = encode_entrypoint_return_record(&vm, &schema).expect("encode typed record");
        let encoded = norito::to_bytes(&record).expect("encode record Norito");
        let decoded: EntrypointReturnRecordV1 =
            norito::decode_from_bytes(&encoded).expect("decode record Norito");
        assert_eq!(decoded, record);
        assert_eq!(
            decoded.schema_hash,
            schema_hash(&schema).expect("schema hash")
        );

        let mut mismatched = schema.clone();
        mismatched.nodes.pop();
        assert!(render_entrypoint_return_record(&mismatched, &decoded).is_err());

        let mut trailing = encoded;
        trailing.push(0);
        assert!(matches!(
            decode_entrypoint_return_record(&schema, &trailing),
            Err(EntrypointReturnDecodeError::RecordEncoding { .. })
        ));
    }

    #[test]
    fn retired_recursive_list_record_encoding_is_rejected() {
        // This test-only encoder preserves the retired field shape solely to
        // prove that the first-release decoder does not accept it. Variant
        // order intentionally matches the canonical enum's discriminants.
        #[derive(Encode)]
        enum LegacyEntrypointValueAtomV1 {
            Tag(bool),
            Int(i64),
            Bool(bool),
            Pointer(Vec<u8>),
            List(Vec<Vec<Self>>),
        }

        #[derive(Encode)]
        struct LegacyEntrypointReturnRecordV1 {
            schema_hash: [u8; 32],
            atoms: Vec<LegacyEntrypointValueAtomV1>,
        }

        let schema = list(1, leaf(EntrypointValueKindV1::Int));
        let schema_hash = schema_hash(&schema).expect("List schema hash");
        let legacy_items = vec![vec![LegacyEntrypointValueAtomV1::Int(7)]];
        let legacy = LegacyEntrypointReturnRecordV1 {
            schema_hash,
            atoms: vec![LegacyEntrypointValueAtomV1::List(legacy_items)],
        };
        // Construct every retired variant so the local encoder's order remains
        // an explicit part of the fixture rather than an accidental omission.
        let _variant_order_guard = (
            LegacyEntrypointValueAtomV1::Tag(false),
            LegacyEntrypointValueAtomV1::Bool(false),
            LegacyEntrypointValueAtomV1::Pointer(Vec::new()),
        );
        let legacy_bytes = norito::to_bytes(&legacy).expect("encode retired recursive shape");
        let canonical_bytes = norito::to_bytes(&EntrypointReturnRecordV1 {
            schema_hash,
            atoms: vec![
                EntrypointValueAtomV1::List(1),
                EntrypointValueAtomV1::Int(7),
            ],
        })
        .expect("encode canonical flat shape");
        assert_ne!(legacy_bytes, canonical_bytes);
        assert!(decode_entrypoint_return_record(&schema, &legacy_bytes).is_err());
    }

    #[test]
    fn exact_return_record_byte_cap_is_inclusive_and_checked_before_encoding() {
        let schema = leaf(EntrypointValueKindV1::Blob);
        let schema_hash = schema_hash(&schema).expect("return schema hash");
        assert_eq!(
            test_tlv(PointerType::NoritoBytes, &[]).len(),
            ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1,
            "the published overhead must match the canonical pointer-ABI encoder"
        );
        let mut payload_len = MAX_ENTRYPOINT_RETURN_RECORD_BYTES.saturating_sub(256);
        let mut record = EntrypointReturnRecordV1 {
            schema_hash,
            atoms: Vec::new(),
        };
        let mut encoded = Vec::new();
        for _ in 0..12 {
            record.atoms = vec![EntrypointValueAtomV1::Pointer(test_tlv(
                PointerType::Blob,
                &vec![0x5A; payload_len],
            ))];
            encoded = norito::to_bytes(&record).expect("encode boundary record");
            match encoded.len().cmp(&MAX_ENTRYPOINT_RETURN_RECORD_BYTES) {
                core::cmp::Ordering::Equal => break,
                core::cmp::Ordering::Less => {
                    payload_len = payload_len.saturating_add(
                        MAX_ENTRYPOINT_RETURN_RECORD_BYTES.saturating_sub(encoded.len()),
                    );
                }
                core::cmp::Ordering::Greater => {
                    payload_len = payload_len
                        .saturating_sub(encoded.len() - MAX_ENTRYPOINT_RETURN_RECORD_BYTES);
                }
            }
        }
        assert_eq!(
            encoded.len(),
            MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
            "fixture must land exactly on the inclusive V1 return boundary"
        );
        assert_eq!(
            encoded.len() + ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1,
            iroha_data_model::smart_contract::entrypoint::MAX_ENTRYPOINT_BOUNDARY_BYTES,
            "record plus its canonical NoritoBytes TLV must fill exactly one boundary envelope"
        );
        assert_eq!(
            exact_record_bytes(&record, MAX_ENTRYPOINT_RETURN_RECORD_BYTES)
                .expect("exact-cap record"),
            encoded
        );
        decode_entrypoint_return_record(&schema, &encoded)
            .expect("the exact canonical V1 cap must decode");

        let boundary_payload = vec![0x5A; payload_len];
        let boundary_envelope = test_tlv(PointerType::Blob, &boundary_payload);
        let mut vm = IVM::new(10_000);
        let pointer = vm
            .alloc_heap(u64::try_from(boundary_envelope.len()).expect("TLV length fits u64"))
            .expect("exact-cap leaf fits the clean child heap");
        vm.store_bytes(pointer, &boundary_envelope)
            .expect("store exact-cap leaf");
        vm.set_register(FIRST_RETURN_REGISTER, pointer);
        assert_eq!(
            encode_entrypoint_return_record_bytes(&vm, &schema)
                .expect("the cumulative clone budget must admit the exact cap"),
            encoded
        );

        let mut oversized = record;
        let EntrypointValueAtomV1::Pointer(envelope) = &mut oversized.atoms[0] else {
            panic!("fixture pointer atom");
        };
        let oversized_payload = vec![0x5A; payload_len + 1];
        *envelope = test_tlv(PointerType::Blob, &oversized_payload);
        assert!(matches!(
            exact_record_bytes(&oversized, MAX_ENTRYPOINT_RETURN_RECORD_BYTES),
            Err(EntrypointReturnDecodeError::RecordTooLarge {
                max_bytes: MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
                ..
            })
        ));
        assert!(matches!(
            decode_entrypoint_return_record(
                &schema,
                &vec![0; MAX_ENTRYPOINT_RETURN_RECORD_BYTES + 1]
            ),
            Err(EntrypointReturnDecodeError::RecordTooLarge {
                bytes,
                max_bytes: MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
            }) if bytes == MAX_ENTRYPOINT_RETURN_RECORD_BYTES + 1
        ));
    }

    #[test]
    fn repeated_large_pointer_is_rejected_before_the_second_clone() {
        let schema = list(2, leaf(EntrypointValueKindV1::Blob));
        let payload = vec![0xA5; MAX_ENTRYPOINT_RETURN_RECORD_BYTES / 2 + 1024];
        let envelope = test_tlv(PointerType::Blob, &payload);
        let mut vm = IVM::new(10_000);
        let pointer = vm
            .alloc_heap(u64::try_from(envelope.len()).expect("TLV length fits u64"))
            .expect("allocate one large public TLV");
        vm.store_bytes(pointer, &envelope)
            .expect("store one large public TLV");
        let layout = ListLayoutV1::try_new(2, 1).expect("list layout");
        let list = ivm::list::allocate_words(&mut vm, layout, &[vec![pointer], vec![pointer]])
            .expect("list repeating one VM pointer");
        vm.set_register(FIRST_RETURN_REGISTER, list);

        assert!(matches!(
            encode_entrypoint_return_record(&vm, &schema),
            Err(EntrypointReturnDecodeError::RecordTooLarge {
                max_bytes: MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
                ..
            })
        ));

        let mut budget = ReturnRecordBudget::default();
        budget
            .reserve_pointer(payload.len())
            .expect("first pointer fits the cumulative clone budget");
        let charged_after_first = budget.lower_bound_bytes;
        assert!(budget.reserve_pointer(payload.len()).is_err());
        assert_eq!(
            budget.lower_bound_bytes, charged_after_first,
            "a rejected pointer must not advance the clone budget"
        );
    }

    #[test]
    fn caller_affordability_bound_rejects_pointer_before_payload_clone() {
        let schema = leaf(EntrypointValueKindV1::Blob);
        let payload = vec![0xA5; 64 * 1024];
        let envelope = test_tlv(PointerType::Blob, &payload);
        let mut vm = IVM::new(10_000);
        let pointer = vm
            .alloc_heap(u64::try_from(envelope.len()).expect("TLV length fits u64"))
            .expect("allocate child return TLV");
        vm.store_bytes(pointer, &envelope)
            .expect("store child return TLV");
        vm.set_register(FIRST_RETURN_REGISTER, pointer);

        let affordable_record_bytes = 1024;
        assert!(matches!(
            encode_entrypoint_return_record_bytes_bounded(
                &vm,
                &schema,
                affordable_record_bytes,
            ),
            Err(EntrypointReturnDecodeError::RecordTooLarge {
                max_bytes,
                ..
            }) if max_bytes == affordable_record_bytes
        ));
    }

    #[test]
    fn inactive_return_record_branches_reject_hidden_atoms() {
        let option_schema = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Blob),
            ],
        };
        let option_record = EntrypointReturnRecordV1 {
            schema_hash: schema_hash(&option_schema).expect("Option schema hash"),
            atoms: vec![
                EntrypointValueAtomV1::Tag(false),
                EntrypointValueAtomV1::Pointer(test_tlv(PointerType::Blob, b"private-placeholder")),
            ],
        };
        assert!(matches!(
            render_entrypoint_return_record(&option_schema, &option_record),
            Err(EntrypointReturnDecodeError::SchemaBinding)
        ));

        let result_schema = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Result,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Blob),
            ],
        };
        let result_record = EntrypointReturnRecordV1 {
            schema_hash: schema_hash(&result_schema).expect("Result schema hash"),
            atoms: vec![
                EntrypointValueAtomV1::Tag(true),
                EntrypointValueAtomV1::Int(7),
                EntrypointValueAtomV1::Pointer(test_tlv(PointerType::Blob, b"private-error")),
            ],
        };
        assert!(matches!(
            render_entrypoint_return_record(&result_schema, &result_record),
            Err(EntrypointReturnDecodeError::SchemaBinding)
        ));
    }

    #[test]
    fn flat_list_records_reject_malformed_truncated_trailing_and_count_mismatch_tapes() {
        let schema = list(2, leaf(EntrypointValueKindV1::Int));
        let valid = EntrypointReturnRecordV1 {
            schema_hash: schema_hash(&schema).expect("List schema hash"),
            atoms: vec![
                EntrypointValueAtomV1::List(2),
                EntrypointValueAtomV1::Int(1),
                EntrypointValueAtomV1::Int(2),
            ],
        };
        assert_eq!(
            render_entrypoint_return_record(&schema, &valid).expect("canonical flat list tape"),
            norito::json!([1, 2])
        );

        for atoms in [
            vec![
                EntrypointValueAtomV1::List(1),
                EntrypointValueAtomV1::Bool(true),
            ],
            vec![
                EntrypointValueAtomV1::List(1),
                EntrypointValueAtomV1::Int(1),
                EntrypointValueAtomV1::Int(2),
            ],
            vec![EntrypointValueAtomV1::List(1)],
            vec![
                EntrypointValueAtomV1::List(2),
                EntrypointValueAtomV1::Int(1),
            ],
            vec![
                EntrypointValueAtomV1::List(0),
                EntrypointValueAtomV1::Int(1),
            ],
            vec![
                EntrypointValueAtomV1::List(3),
                EntrypointValueAtomV1::Int(1),
                EntrypointValueAtomV1::Int(2),
                EntrypointValueAtomV1::Int(3),
            ],
        ] {
            let record = EntrypointReturnRecordV1 {
                schema_hash: schema_hash(&schema).expect("List schema hash"),
                atoms,
            };
            assert!(render_entrypoint_return_record(&schema, &record).is_err());
            let encoded = norito::to_bytes(&record).expect("encode malformed tape fixture");
            assert!(decode_entrypoint_return_record(&schema, &encoded).is_err());
        }
    }

    #[test]
    fn malformed_tags_and_booleans_fail_closed_without_reading_inactive_words() {
        let mut vm = IVM::new(10_000);
        vm.set_register(FIRST_RETURN_REGISTER, 2);
        assert!(matches!(
            decode_entrypoint_return(&vm, &leaf(EntrypointValueKindV1::Bool)),
            Err(EntrypointReturnDecodeError::NonCanonicalBit { role: "bool", .. })
        ));

        let option_int = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            ],
        };
        let forged = ivm::sum::allocate_words(
            &mut vm,
            SumLayoutV1::option(1).expect("Option layout"),
            0,
            &[],
        )
        .expect("Option::none to forge");
        vm.store_u64(forged, 2).expect("forge Option tag");
        vm.set_register(FIRST_RETURN_REGISTER, forged);
        assert!(matches!(
            decode_entrypoint_return(&vm, &option_int),
            Err(EntrypointReturnDecodeError::InvalidValue { kind: "Option", .. })
        ));
        let none = ivm::sum::allocate_words(
            &mut vm,
            SumLayoutV1::option(1).expect("Option layout"),
            0,
            &[],
        )
        .expect("Option::none");
        vm.set_register(FIRST_RETURN_REGISTER, none);
        vm.store_u64(none + 8, 99)
            .expect("forge inactive Option storage");
        assert!(matches!(
            decode_entrypoint_return(&vm, &option_int),
            Err(EntrypointReturnDecodeError::InvalidValue { kind: "Option", .. })
        ));
        vm.store_u64(none + 8, 0)
            .expect("restore canonical inactive Option storage");
        vm.set_register(FIRST_RETURN_REGISTER + 1, 7);
        assert_eq!(
            decode_entrypoint_return(&vm, &option_int).expect("decode active-only None"),
            norito::json!({ "none": true })
        );
    }

    #[test]
    fn private_inactive_words_are_ignored_but_active_words_are_rejected() {
        let option_string = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
            ],
        };
        let mut vm = IVM::new(10_000);
        vm.set_zk_mode(true);
        let none = ivm::sum::allocate_words(
            &mut vm,
            SumLayoutV1::option(1).expect("Option layout"),
            0,
            &[],
        )
        .expect("Option::none");
        vm.set_register(FIRST_RETURN_REGISTER, none);
        vm.set_register(FIRST_RETURN_REGISTER + 1, 0);
        vm.registers.set_tag(FIRST_RETURN_REGISTER + 1, true);
        assert_eq!(
            decode_entrypoint_return(&vm, &option_string)
                .expect("inactive private register is not decoded"),
            norito::json!({ "none": true })
        );

        let pointer = input_tlv(&mut vm, PointerType::Blob, b"secret");
        let some = ivm::sum::allocate_words(
            &mut vm,
            SumLayoutV1::option(1).expect("Option layout"),
            1,
            &[pointer],
        )
        .expect("Option::some");
        vm.set_register(FIRST_RETURN_REGISTER, some);
        vm.registers.set_tag(FIRST_RETURN_REGISTER, true);
        assert!(matches!(
            decode_entrypoint_return(&vm, &option_string),
            Err(EntrypointReturnDecodeError::Privacy { .. })
        ));
    }

    #[test]
    fn nested_amount_list_roundtrips_as_one_return_word() {
        let schema = list(2, list(2, leaf(EntrypointValueKindV1::Amount)));
        let mut vm = IVM::new(10_000);
        let amount_payload = norito::to_bytes(&Numeric::new(125, 2)).expect("Amount payload");
        let amount_pointer = input_tlv(&mut vm, PointerType::Quantity, &amount_payload);
        let inner_layout = ListLayoutV1::try_new(2, 1).expect("inner layout");
        let first = ivm::list::allocate_words(&mut vm, inner_layout, &[vec![amount_pointer]])
            .expect("first inner list");
        let second = ivm::list::allocate_words(&mut vm, inner_layout, &[vec![amount_pointer]])
            .expect("second inner list");
        let outer_layout = ListLayoutV1::try_new(2, 1).expect("outer layout");
        let list = ivm::list::allocate_words(&mut vm, outer_layout, &[vec![first], vec![second]])
            .expect("outer list");
        vm.set_register(FIRST_RETURN_REGISTER, list);
        assert_eq!(
            decode_entrypoint_return(&vm, &schema).expect("decode nested Amount list"),
            norito::json!([["1.25"], ["1.25"]])
        );

        let overflow =
            ivm::list::allocate_words(&mut vm, outer_layout, &[vec![first], vec![second]])
                .expect("outer list to forge");
        vm.store_u64(overflow, 3)
            .expect("forge length past capacity");
        vm.set_register(FIRST_RETURN_REGISTER, overflow);
        assert!(matches!(
            encode_entrypoint_return_record(&vm, &schema),
            Err(EntrypointReturnDecodeError::InvalidValue { kind: "List", .. })
        ));

        let wrong = input_tlv(&mut vm, PointerType::Blob, &amount_payload);
        let wrong_inner = ivm::list::allocate_words(&mut vm, inner_layout, &[vec![wrong]])
            .expect("inner list with wrong pointer type");
        let wrong_outer = ivm::list::allocate_words(&mut vm, outer_layout, &[vec![wrong_inner]])
            .expect("outer list with wrong pointer type");
        vm.set_register(FIRST_RETURN_REGISTER, wrong_outer);
        assert!(matches!(
            encode_entrypoint_return_record(&vm, &schema),
            Err(EntrypointReturnDecodeError::PointerType {
                expected: PointerType::Quantity,
                actual: PointerType::Blob,
                ..
            })
        ));
    }

    #[test]
    fn list_of_nested_option_results_decodes_active_only_sum_handles() {
        let element = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Result,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Amount),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
            ],
        };
        let schema = list(3, element);
        let mut vm = IVM::new(10_000);
        let amount_payload = norito::to_bytes(&Numeric::new(125, 2)).expect("Amount payload");
        let amount = input_tlv(&mut vm, PointerType::Quantity, &amount_payload);
        let result_layout = SumLayoutV1::try_new(1, 1).expect("Result layout");
        let option_layout = SumLayoutV1::option(1).expect("Option layout");
        let ok =
            ivm::sum::allocate_words(&mut vm, result_layout, 1, &[amount]).expect("Result::ok");
        let some_ok =
            ivm::sum::allocate_words(&mut vm, option_layout, 1, &[ok]).expect("Option::some ok");
        let err = ivm::sum::allocate_words(&mut vm, result_layout, 0, &[1]).expect("Result::err");
        let some_err =
            ivm::sum::allocate_words(&mut vm, option_layout, 1, &[err]).expect("Option::some err");
        let none = ivm::sum::allocate_words(&mut vm, option_layout, 0, &[]).expect("Option::none");
        let list_layout = ListLayoutV1::try_new(3, 1).expect("list layout");
        let list = ivm::list::allocate_words(
            &mut vm,
            list_layout,
            &[vec![some_ok], vec![some_err], vec![none]],
        )
        .expect("allocate list");
        vm.set_register(FIRST_RETURN_REGISTER, list);
        assert_eq!(
            decode_entrypoint_return(&vm, &schema).expect("decode nested sums"),
            norito::json!([
                { "some": { "ok": "1.25" } },
                { "some": { "err": true } },
                { "none": true },
            ])
        );

        let forged =
            ivm::sum::allocate_words(&mut vm, result_layout, 0, &[1]).expect("Result to forge");
        vm.store_u64(forged, 2).expect("forge Result tag");
        let some_forged = ivm::sum::allocate_words(&mut vm, option_layout, 1, &[forged])
            .expect("Option with forged Result");
        let forged_list = ivm::list::allocate_words(&mut vm, list_layout, &[vec![some_forged]])
            .expect("list with forged Result");
        vm.set_register(FIRST_RETURN_REGISTER, forged_list);
        assert!(matches!(
            encode_entrypoint_return_record(&vm, &schema),
            Err(EntrypointReturnDecodeError::InvalidValue { kind: "Result", .. })
        ));
    }
}
