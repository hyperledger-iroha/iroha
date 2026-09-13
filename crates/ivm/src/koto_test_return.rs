//! Schema-bound ownership transfer from a nested Kotodama test VM.
use super::*;
use crate::{VMError, list::ListLayoutV1, sum::SumLayoutV1};
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
fn expected_mask(schema: &EntrypointValueTypeV1) -> Result<u64, VMError> {
    let mut pending = vec![0];
    let mut count = 0;
    let mut mask = 0;
    while let Some(at) = pending.pop() {
        match &schema.nodes[at] {
            Node::Tuple(_) | Node::Struct(_) => {
                pending.extend(child_nodes(schema, at)?.into_iter().rev())
            }
            node => {
                if !matches!(node, Node::Unit | Node::Error(_) | Node::Leaf(Kind::Bool)) {
                    mask |= 1 << count;
                }
                count += 1;
            }
        }
    }
    Ok(mask)
}
fn charge(used: &mut usize, bytes: usize) -> Result<(), VMError> {
    *used = used
        .checked_add(bytes)
        .filter(|next| *next <= MAX_ENTRYPOINT_RETURN_RECORD_BYTES)
        .ok_or_else(invalid)?;
    Ok(())
}

pub(super) fn transfer_return(
    source: &IVM,
    destination: &mut IVM,
    schema: &EntrypointValueTypeV1,
    arity: usize,
    mask: u64,
) -> Result<(), VMError> {
    if !schema.validate()
        || schema.word_count() != Some(arity)
        || arity > TEST_MAX_RETURN_VALUES
        || expected_mask(schema)? != mask
    {
        return Err(invalid());
    }
    let words = (0..arity)
        .map(|index| {
            source.ensure_public_register(10 + index)?;
            Ok(source.register(10 + index))
        })
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
        destination.set_register(10 + index, output[*value]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_primitives::numeric_abi::IntValueV1;
    use ivm_abi::entrypoint::EntrypointListTypeNodeV1;

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
        source.set_register(10, some);
        let mut destination = IVM::new(0);
        destination.alloc_heap(128).unwrap();
        transfer_return(&source, &mut destination, &schema, 1, 1).unwrap();
        drop(source);
        let (tag, active) =
            crate::sum::read_words(&destination, destination.register(10), option).unwrap();
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
        source.set_register(10, none);
        let mut destination = IVM::new(0);
        destination.set_register(10, 42);
        assert!(transfer_return(&source, &mut destination, &schema, 2, 1).is_err());
        assert!(transfer_return(&source, &mut destination, &schema, 1, 0).is_err());
        source.store_u64(none + 8, 99).unwrap();
        assert!(transfer_return(&source, &mut destination, &schema, 1, 1).is_err());
        assert_eq!(destination.register(10), 42);
        assert_eq!(
            destination.alloc_heap(8).unwrap(),
            crate::memory::Memory::HEAP_START
        );
        source.store_u64(none + 8, 0).unwrap();
        transfer_return(&source, &mut destination, &schema, 1, 1).unwrap();
        assert_eq!(
            crate::sum::read_words(&destination, destination.register(10), layout).unwrap(),
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
        source.set_register(10, 2);
        let wrong = source
            .alloc_host_tlv(&make_tlv(PointerType::Blob, b"wrong"))
            .unwrap();
        source.set_register(11, wrong);
        let mut destination = IVM::new(0);
        assert!(transfer_return(&source, &mut destination, &schema, 2, 2).is_err());
        source.set_register(10, 1);
        assert!(transfer_return(&source, &mut destination, &schema, 2, 2).is_err());
        let integer = source
            .alloc_host_tlv(&crate::numeric_tlv::encode_int(&7.into()).unwrap())
            .unwrap();
        source.set_register(11, integer);
        transfer_return(&source, &mut destination, &schema, 2, 2).unwrap();
        assert_eq!(destination.register(10), 1);
        assert_eq!(
            destination
                .validate_tlv(destination.register(11))
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
        source.set_register(10, 0);
        transfer_return(&source, &mut destination, &schema, 1, 0).unwrap();
        assert_eq!(destination.register(10), 0);
        let layout = SumLayoutV1::option(1).unwrap();
        let mut handle = 0;
        for _ in 1..ivm_abi::entrypoint::MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH {
            schema.nodes.insert(0, Node::Option);
            handle = crate::sum::allocate_words(&mut source, layout, 1, &[handle]).unwrap();
        }
        source.set_register(10, handle);
        transfer_return(&source, &mut destination, &schema, 1, 1).unwrap();
        drop(source);
        let mut handle = destination.register(10);
        for _ in 1..ivm_abi::entrypoint::MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH {
            let (tag, payload) = crate::sum::read_words(&destination, handle, layout).unwrap();
            assert!(tag);
            handle = payload[0];
        }
        assert_eq!(handle, 0);
    }
}
