//! Schema-bound ownership transfer from a nested Kotodama test VM.
//!
//! The Kotodama test harness (`kotodama_toolchain`) runs `test::invoke_kotoage_as` calls in a
//! nested VM and copies the typed public call result into the caller's result table. The copy
//! needs VM-private memory state (call frames, HEAP ownership), so it lives here.
use crate::{
    AccountId, AssetDefinitionId, IVM, PointerType, VMError, list::ListLayoutV1, sum::SumLayoutV1,
};
use iroha_data_model::asset::AssetId;
use iroha_model_base::{domain::DomainId, name::Name, topology::DataSpaceId};
use iroha_primitives::{json::Json, numeric_abi::DecimalValueV1};
use ivm_abi::entrypoint::{
    EntrypointValueKindV1 as Kind, EntrypointValueTypeNodeV1 as Node, EntrypointValueTypeV1,
    MAX_ENTRYPOINT_RETURN_RECORD_BYTES, entrypoint_value_subtree_range_v1,
};

// Flat postorder plans keep both traversal and destruction independent of schema depth.
enum CopyValue {
    Scalar(u64),
    Tlv(Vec<u8>),
    Sum(SumLayoutV1, bool, Vec<usize>),
    List(ListLayoutV1, Vec<Vec<usize>>),
}
enum Task {
    Visit(usize, Vec<u64>),
    Product(usize),
    Sum(usize, SumLayoutV1, bool),
    List(usize, ListLayoutV1),
}
fn invalid() -> VMError {
    VMError::DecodeError
}
fn child_nodes(schema: &EntrypointValueTypeV1, at: usize) -> Result<Vec<usize>, VMError> {
    let count = match &schema.nodes[at] {
        Node::Struct(node) => node.fields.len(),
        Node::Tuple(count) => usize::from(*count),
        Node::Option | Node::List(_) => 1,
        Node::Result => 2,
        _ => 0,
    };
    let mut child = at + 1;
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        result.push(child);
        child = entrypoint_value_subtree_range_v1(&schema.nodes, child)
            .ok_or_else(invalid)?
            .end;
    }
    Ok(result)
}
fn width(schema: &EntrypointValueTypeV1, at: usize) -> Result<usize, VMError> {
    let range = entrypoint_value_subtree_range_v1(&schema.nodes, at).ok_or_else(invalid)?;
    EntrypointValueTypeV1 {
        nodes: schema.nodes[range].to_vec(),
    }
    .word_count()
    .ok_or_else(invalid)
}
fn pointer_type(kind: Kind) -> Option<PointerType> {
    Some(match kind {
        Kind::Bool => return None,
        Kind::Int => PointerType::Int,
        Kind::Decimal => PointerType::Decimal,
        Kind::Quantity => PointerType::Quantity,
        Kind::String | Kind::Blob => PointerType::Blob,
        Kind::Json => PointerType::Json,
        Kind::Name => PointerType::Name,
        Kind::AccountId => PointerType::AccountId,
        Kind::AssetDefinitionId => PointerType::AssetDefinitionId,
        Kind::AssetId => PointerType::AssetId,
        Kind::DomainId => PointerType::DomainId,
        Kind::NftId => PointerType::NftId,
        Kind::DataSpaceId => PointerType::DataSpaceId,
    })
}
fn validate_leaf(kind: Kind, bytes: &[u8]) -> Result<(), VMError> {
    macro_rules! canonical {
        ($ty:ty) => {
            norito::decode_canonical::<$ty>(bytes)
                .map(|_| ())
                .map_err(|_| invalid())
        };
    }
    match kind {
        Kind::Int => iroha_primitives::numeric_abi::IntValueV1::decode_frame(bytes)
            .map(|_| ())
            .map_err(|_| invalid()),
        Kind::Decimal => DecimalValueV1::decode_frame(bytes)
            .map(|_| ())
            .map_err(|_| invalid()),
        Kind::Quantity => iroha_primitives::numeric_abi::QuantityValueV1::decode_frame(bytes)
            .map(|_| ())
            .map_err(|_| invalid()),
        Kind::String => std::str::from_utf8(bytes)
            .map(|_| ())
            .map_err(|_| invalid()),
        Kind::Blob => Ok(()),
        Kind::Json => canonical!(Json),
        Kind::Name => canonical!(Name),
        Kind::AccountId => canonical!(AccountId),
        Kind::AssetDefinitionId => canonical!(AssetDefinitionId),
        Kind::AssetId => canonical!(AssetId),
        Kind::DomainId => canonical!(DomainId),
        Kind::NftId => canonical!(iroha_data_model::nft::NftId),
        Kind::DataSpaceId => canonical!(DataSpaceId),
        Kind::Bool => Err(invalid()),
    }
}
fn charge(used: &mut usize, bytes: usize) -> Result<(), VMError> {
    *used = used
        .checked_add(bytes)
        .filter(|next| *next <= MAX_ENTRYPOINT_RETURN_RECORD_BYTES)
        .ok_or_else(invalid)?;
    Ok(())
}

/// Copy the schema-typed public call result of a completed `source` VM into `destination`.
///
/// Values are revalidated against `schema`, TLVs and HEAP handles are preflighted before any
/// allocation, and the result table address plus arity are published in `x10`/`x11`.
///
/// # Errors
///
/// Returns [`VMError`] when the schema, arity or result table is invalid, a value does not match
/// its schema, or `destination` lacks the INPUT/HEAP capacity for the copied values.
pub fn transfer_return(
    source: &IVM,
    destination: &mut IVM,
    schema: &EntrypointValueTypeV1,
    arity: usize,
    result_table: u64,
) -> Result<(), VMError> {
    if !schema.validate()
        || schema.word_count() != Some(arity)
        || arity == 0
        || arity > ivm_abi::call::MAX_CALL_WORDS_V1
        || source.call_result_word_count()? != arity
    {
        return Err(invalid());
    }
    if !result_table.is_multiple_of(ivm_abi::call::CALL_WORD_BYTES_V1 as u64) {
        return Err(VMError::MisalignedAccess {
            addr: result_table as u32,
        });
    }
    let result_bytes = (arity * ivm_abi::call::CALL_WORD_BYTES_V1) as u64;
    let result_end = result_table.checked_add(result_bytes).ok_or_else(invalid)?;
    let allocated_heap = result_table >= crate::Memory::HEAP_START
        && result_end <= crate::Memory::HEAP_START + destination.memory.heap_allocated_len();
    let active_stack = result_table >= crate::Memory::STACK_START
        && destination.memory.call_frames.entry_pc().is_ok();
    if !allocated_heap && !active_stack {
        return Err(invalid());
    }
    destination.memory.checked_region_bounds_for(
        result_table,
        result_bytes,
        crate::error::Perm::WRITE,
    )?;
    let words = (0..arity)
        .map(|index| source.public_call_result_word(index))
        .collect::<Result<Vec<_>, VMError>>()?;
    let mut tasks = vec![Task::Visit(0, words)];
    let mut completed: Vec<Vec<usize>> = Vec::new();
    let mut plan = Vec::new();
    let mut bytes = 0;
    while let Some(task) = tasks.pop() {
        let value = match task {
            Task::Product(start) => {
                let children = completed.split_off(start);
                completed.push(children.into_iter().flatten().collect());
                continue;
            }
            Task::Sum(start, layout, tag) => CopyValue::Sum(
                layout,
                tag,
                completed.split_off(start).into_iter().flatten().collect(),
            ),
            Task::List(start, layout) => CopyValue::List(layout, completed.split_off(start)),
            Task::Visit(at, words) => {
                charge(&mut bytes, 8)?;
                if words.len() != width(schema, at)? {
                    return Err(invalid());
                }
                let children = child_nodes(schema, at)?;
                let first = words.first().copied().ok_or_else(invalid)?;
                match &schema.nodes[at] {
                    Node::Struct(node) if node.fields.is_empty() => {
                        if first != 0 {
                            return Err(invalid());
                        }
                        CopyValue::Scalar(0)
                    }
                    Node::Tuple(_) | Node::Struct(_) => {
                        tasks.push(Task::Product(completed.len()));
                        let mut offset = 0;
                        let mut next = Vec::new();
                        for child in children {
                            let end = offset + width(schema, child)?;
                            next.push(Task::Visit(child, words[offset..end].to_vec()));
                            offset = end;
                        }
                        tasks.extend(next.into_iter().rev());
                        continue;
                    }
                    Node::Option | Node::Result => {
                        let layout = if matches!(&schema.nodes[at], Node::Option) {
                            SumLayoutV1::option(width(schema, children[0])? as u64)
                        } else {
                            SumLayoutV1::try_new(
                                width(schema, children[1])? as u64,
                                width(schema, children[0])? as u64,
                            )
                        }
                        .map_err(|_| invalid())?;
                        charge(
                            &mut bytes,
                            usize::try_from(layout.allocation_bytes().map_err(|_| invalid())?)
                                .map_err(|_| invalid())?,
                        )?;
                        let (tag, active) = crate::sum::read_words(source, first, layout)?;
                        tasks.push(Task::Sum(completed.len(), layout, tag));
                        if tag {
                            tasks.push(Task::Visit(children[0], active));
                        } else if children.len() == 2 {
                            tasks.push(Task::Visit(children[1], active));
                        }
                        continue;
                    }
                    Node::List(node) => {
                        let layout = ListLayoutV1::try_new(
                            u64::from(node.capacity),
                            width(schema, children[0])? as u64,
                        )
                        .map_err(|_| invalid())?;
                        charge(
                            &mut bytes,
                            usize::try_from(layout.allocation_bytes().map_err(|_| invalid())?)
                                .map_err(|_| invalid())?,
                        )?;
                        let elements = crate::list::read_words(source, first, layout)?;
                        tasks.push(Task::List(completed.len(), layout));
                        tasks.extend(
                            elements
                                .into_iter()
                                .rev()
                                .map(|words| Task::Visit(children[0], words)),
                        );
                        continue;
                    }
                    Node::Unit if first == 0 => CopyValue::Scalar(first),
                    Node::Error(error)
                        if u32::try_from(first)
                            .ok()
                            .and_then(|code| error.variant(code))
                            .is_some() =>
                    {
                        CopyValue::Scalar(first)
                    }
                    Node::Leaf(Kind::Bool) if first <= 1 => CopyValue::Scalar(first),
                    Node::Leaf(kind) => {
                        let tlv = source.validate_tlv(first)?;
                        if Some(tlv.type_id) != pointer_type(*kind) {
                            return Err(invalid());
                        }
                        charge(&mut bytes, 39 + tlv.payload.len())?;
                        validate_leaf(*kind, tlv.payload)?;
                        CopyValue::Tlv(source.clone_tlv(first)?)
                    }
                    Node::StateCursor(key) => {
                        let tlv = source.validate_tlv(first)?;
                        charge(&mut bytes, 39 + tlv.payload.len())?;
                        let envelope = source.clone_tlv(first)?;
                        crate::state_cursor::validate_cursor_envelope(*key, &envelope)?;
                        CopyValue::Tlv(envelope)
                    }
                    _ => return Err(invalid()),
                }
            }
        };
        completed.push(vec![plan.len()]);
        plan.push(value);
    }
    if completed.len() != 1 || completed[0].len() != arity {
        return Err(invalid());
    }
    let mut lengths = Vec::new();
    let mut raw_heap = 0u64;
    for value in &plan {
        match value {
            CopyValue::Tlv(bytes) => lengths.push(bytes.len()),
            CopyValue::Sum(layout, ..) => {
                raw_heap = raw_heap
                    .checked_add(layout.allocation_bytes().map_err(|_| invalid())?)
                    .ok_or_else(invalid)?
            }
            CopyValue::List(layout, ..) => {
                raw_heap = raw_heap
                    .checked_add(layout.allocation_bytes().map_err(|_| invalid())?)
                    .ok_or_else(invalid)?
            }
            CopyValue::Scalar(_) => {}
        }
    }
    destination.preflight_host_tlv_allocations_with_reserved_heap(&lengths, raw_heap)?;
    // Allocate every TLV before raw handles, exactly as the allocation preflight models.
    let mut output = vec![0; plan.len()];
    for (index, value) in plan.iter().enumerate() {
        if let CopyValue::Tlv(bytes) = value {
            output[index] = destination.alloc_host_tlv(bytes)?;
        }
    }
    for (index, value) in plan.iter().enumerate() {
        output[index] = match value {
            CopyValue::Scalar(value) => *value,
            CopyValue::Tlv(_) => output[index],
            CopyValue::Sum(layout, tag, children) => crate::sum::allocate_words(
                destination,
                *layout,
                u64::from(*tag),
                &children
                    .iter()
                    .map(|child| output[*child])
                    .collect::<Vec<_>>(),
            )?,
            CopyValue::List(layout, elements) => crate::list::allocate_words(
                destination,
                *layout,
                &elements
                    .iter()
                    .map(|children| children.iter().map(|child| output[*child]).collect())
                    .collect::<Vec<_>>(),
            )?,
        };
    }
    for (index, value) in completed[0].iter().enumerate() {
        destination.store_u64(result_table + (index * 8) as u64, output[*value])?;
    }
    destination.set_register(10, result_table);
    destination.set_register(11, arity as u64);
    Ok(())
}

#[cfg(test)]
fn complete_test_result(source: &mut IVM, words: &[u64]) -> u64 {
    let result_table = source.alloc_heap((words.len() * 8) as u64).unwrap();
    let stack_top = source.memory.stack_top();
    let callable = crate::call_frame::CallFrameShape {
        entry_pc: 0,
        frame_bytes: 0,
        argument_words: 0,
        result_words: words.len(),
    };
    source
        .memory
        .call_frames
        .enter_root(
            stack_top,
            &callable,
            crate::call_frame::CallTables {
                argument_base: 0,
                argument_words: 0,
                result_base: result_table,
                result_words: words.len() as u64,
            },
            stack_top,
        )
        .unwrap();
    for (index, word) in words.iter().enumerate() {
        source
            .store_u64(result_table + (index * 8) as u64, *word)
            .unwrap();
    }
    // These unit tests exercise schema validation independently of root runtime role validation.
    source
        .memory
        .call_frames
        .finish(stack_top, result_table, words.len() as u64)
        .unwrap();
    result_table
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_primitives::numeric_abi::IntValueV1;
    use ivm_abi::entrypoint::EntrypointListTypeNodeV1;

    fn make_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
        out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
        out.push(1);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(payload);
        let hash: [u8; 32] = iroha_crypto::Hash::new(payload).into();
        out.extend_from_slice(&hash);
        out
    }

    #[test]
    fn nested_sum_list_returns_own_all_handles_and_tlvs() {
        let schema = EntrypointValueTypeV1 {
            nodes: vec![
                Node::Option,
                Node::List(EntrypointListTypeNodeV1 { capacity: 2 }),
                Node::Result,
                Node::Leaf(Kind::Int),
                Node::Leaf(Kind::Bool),
            ],
        };
        let sum = SumLayoutV1::try_new(1, 1).unwrap();
        let list = ListLayoutV1::try_new(2, 1).unwrap();
        let option = SumLayoutV1::option(1).unwrap();
        let mut source = IVM::new(0);
        let integer = source
            .alloc_host_tlv(&crate::numeric_tlv::encode_int(&30.into()).unwrap())
            .unwrap();
        let ok = crate::sum::allocate_words(&mut source, sum, 1, &[integer]).unwrap();
        let err = crate::sum::allocate_words(&mut source, sum, 0, &[1]).unwrap();
        let items = crate::list::allocate_words(&mut source, list, &[vec![ok], vec![err]]).unwrap();
        let some = crate::sum::allocate_words(&mut source, option, 1, &[items]).unwrap();
        complete_test_result(&mut source, &[some]);
        let mut destination = IVM::new(0);
        destination.alloc_heap(128).unwrap();
        let result_table = destination.alloc_heap(8).unwrap();
        transfer_return(&source, &mut destination, &schema, 1, result_table).unwrap();
        drop(source);
        let (tag, active) = crate::sum::read_words(
            &destination,
            destination.load_u64(result_table).unwrap(),
            option,
        )
        .unwrap();
        assert!(tag);
        let elements = crate::list::read_words(&destination, active[0], list).unwrap();
        assert_eq!(elements.len(), 2);
        let (tag, active) = crate::sum::read_words(&destination, elements[0][0], sum).unwrap();
        assert!(tag);
        let value = destination.validate_tlv(active[0]).unwrap();
        assert_eq!(value.type_id, PointerType::Int);
        assert_eq!(
            IntValueV1::decode_frame(value.payload)
                .unwrap()
                .into_int()
                .try_to_i64(),
            Some(30)
        );
        assert_eq!(
            crate::sum::read_words(&destination, elements[1][0], sum).unwrap(),
            (false, vec![1])
        );
    }

    #[test]
    fn malformed_return_shape_or_handle_cannot_mutate_destination() {
        let schema = EntrypointValueTypeV1 {
            nodes: vec![Node::Option, Node::Leaf(Kind::Int)],
        };
        let layout = SumLayoutV1::option(1).unwrap();
        let mut source = IVM::new(0);
        let none = crate::sum::allocate_words(&mut source, layout, 0, &[]).unwrap();
        complete_test_result(&mut source, &[none]);
        let mut destination = IVM::new(0);
        destination.set_register(10, 42);
        let result_table = destination.alloc_heap(8).unwrap();
        destination.store_u64(result_table, 99).unwrap();
        let allocated = destination.memory.heap_allocated_len();
        assert!(transfer_return(&source, &mut destination, &schema, 2, result_table).is_err());
        assert!(transfer_return(&source, &mut destination, &schema, 1, result_table + 1).is_err());
        assert!(transfer_return(&source, &mut destination, &schema, 1, 0).is_err());
        for unowned in [
            result_table + 8,
            crate::Memory::OUTPUT_START,
            destination.memory.stack_top() - 8,
        ] {
            assert!(transfer_return(&source, &mut destination, &schema, 1, unowned).is_err());
        }
        source.store_u64(none + 8, 99).unwrap();
        assert!(transfer_return(&source, &mut destination, &schema, 1, result_table).is_err());
        assert_eq!(destination.register(10), 42);
        assert_eq!(destination.memory.heap_allocated_len(), allocated);
        assert_eq!(destination.load_u64(result_table).unwrap(), 99);
        source.store_u64(none + 8, 0).unwrap();
        transfer_return(&source, &mut destination, &schema, 1, result_table).unwrap();
        assert_eq!(
            crate::sum::read_words(
                &destination,
                destination.load_u64(result_table).unwrap(),
                layout
            )
            .unwrap(),
            (false, vec![])
        );
    }

    #[test]
    fn tuple_transfer_validates_scalars_and_all_nested_pointer_types() {
        let schema = EntrypointValueTypeV1 {
            nodes: vec![
                Node::Tuple(2),
                Node::Leaf(Kind::Bool),
                Node::Leaf(Kind::Int),
            ],
        };
        let mut source = IVM::new(0);
        let wrong = source
            .alloc_host_tlv(&make_tlv(PointerType::Blob, b"wrong"))
            .unwrap();
        let source_table = complete_test_result(&mut source, &[2, wrong]);
        let mut destination = IVM::new(0);
        let result_table = destination.alloc_heap(16).unwrap();
        assert!(transfer_return(&source, &mut destination, &schema, 2, result_table).is_err());
        source.store_u64(source_table, 1).unwrap();
        assert!(transfer_return(&source, &mut destination, &schema, 2, result_table).is_err());
        let integer = source
            .alloc_host_tlv(&crate::numeric_tlv::encode_int(&7.into()).unwrap())
            .unwrap();
        source.store_u64(source_table + 8, integer).unwrap();
        transfer_return(&source, &mut destination, &schema, 2, result_table).unwrap();
        assert_eq!(destination.load_u64(result_table).unwrap(), 1);
        assert_eq!(
            destination
                .validate_tlv(destination.load_u64(result_table + 8).unwrap())
                .unwrap()
                .type_id,
            PointerType::Int
        );
    }
}

#[cfg(test)]
mod depth_tests {
    use super::*;
    #[test]
    fn maximal_option_depth_and_unit_word_transfer_without_recursion() {
        let mut schema = EntrypointValueTypeV1 {
            nodes: vec![Node::Unit],
        };
        let mut source = IVM::new(0);
        let mut destination = IVM::new(0);
        let source_table = complete_test_result(&mut source, &[0]);
        let result_table = destination.alloc_heap(8).unwrap();
        transfer_return(&source, &mut destination, &schema, 1, result_table).unwrap();
        assert_eq!(destination.load_u64(result_table).unwrap(), 0);
        let layout = SumLayoutV1::option(1).unwrap();
        let mut handle = 0;
        for _ in 1..ivm_abi::entrypoint::MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH {
            schema.nodes.insert(0, Node::Option);
            handle = crate::sum::allocate_words(&mut source, layout, 1, &[handle]).unwrap();
        }
        source.store_u64(source_table, handle).unwrap();
        transfer_return(&source, &mut destination, &schema, 1, result_table).unwrap();
        drop(source);
        let mut handle = destination.load_u64(result_table).unwrap();
        for _ in 1..ivm_abi::entrypoint::MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH {
            let (tag, payload) = crate::sum::read_words(&destination, handle, layout).unwrap();
            assert!(tag);
            handle = payload[0];
        }
        assert_eq!(handle, 0);
    }
}
