//! Exact prepaid immutable replacement paths for the resident commitment owner.

use mv::allocation::AllocationReservation;

use super::{
    AllocationBudget, AllocationRefusal, ChargedShared, Hash, MerkleMapError, Node, NodeKind,
    PrepaidSharedError, branch_hash, common_bits, key_bit, leaf_hash, prefix,
};

type Owner = ChargedShared<Node>;

/// Inspect at most one public key path without allocating or reading value bytes.
pub(super) fn replacement_nodes(node: Option<&Owner>, key: &Hash, after: Option<Hash>) -> usize {
    let Some(mut node) = node else {
        return usize::from(after.is_some());
    };
    let mut ancestors = 0;
    loop {
        let shared = common_bits(key, &node.first);
        let depth = match &node.kind {
            NodeKind::Leaf(_) => 256,
            NodeKind::Branch { bit, .. } => *bit,
        };
        if shared < depth {
            return ancestors + 2;
        }
        match &node.kind {
            NodeKind::Leaf(_) => {
                return if after.is_some() {
                    ancestors + 1
                } else {
                    ancestors.saturating_sub(1)
                };
            }
            NodeKind::Branch { bit, left, right } => {
                ancestors += 1;
                node = if key_bit(key, *bit) { right } else { left };
            }
        }
    }
}

pub(super) fn prepare(
    node: Option<&Owner>,
    key: Hash,
    after: Option<Hash>,
    budget: &AllocationBudget,
) -> Result<Option<Owner>, MerkleMapError> {
    let count = replacement_nodes(node, &key, after);
    let bytes = Owner::allocation_layout()
        .size()
        .checked_mul(count)
        .ok_or(MerkleMapError::Admission(AllocationRefusal::DemandOverflow))?;
    let mut reservation = budget
        .try_reserve_bytes(bytes)
        .map_err(MerkleMapError::Admission)?;
    let replacement =
        replace_node(node, key, after, &mut reservation).map_err(MerkleMapError::Allocation)?;
    debug_assert_eq!(reservation.remaining_bytes(), 0);
    Ok(replacement)
}

fn allocated(
    node: Node,
    reservation: &mut AllocationReservation,
) -> Result<Owner, PrepaidSharedError> {
    Owner::from_reservation(node, reservation).map_err(|(_, error)| error)
}

fn leaf(
    key: Hash,
    value: Hash,
    reservation: &mut AllocationReservation,
) -> Result<Owner, PrepaidSharedError> {
    allocated(
        Node {
            hash: leaf_hash(key, value),
            first: key,
            kind: NodeKind::Leaf(value),
        },
        reservation,
    )
}

fn branch(
    bit: u16,
    left: Owner,
    right: Owner,
    reservation: &mut AllocationReservation,
) -> Result<Owner, PrepaidSharedError> {
    allocated(
        Node {
            hash: branch_hash(bit, &prefix(&left.first, bit), left.hash, right.hash),
            first: left.first,
            kind: NodeKind::Branch { bit, left, right },
        },
        reservation,
    )
}

fn replace_node(
    node: Option<&Owner>,
    key: Hash,
    after: Option<Hash>,
    reservation: &mut AllocationReservation,
) -> Result<Option<Owner>, PrepaidSharedError> {
    let Some(node) = node else {
        return after.map(|value| leaf(key, value, reservation)).transpose();
    };
    let shared = common_bits(&key, &node.first);
    let depth = match &node.kind {
        NodeKind::Leaf(_) => 256,
        NodeKind::Branch { bit, .. } => *bit,
    };
    if shared < depth {
        return after
            .map(|value| {
                let new = leaf(key, value, reservation)?;
                if key_bit(&key, shared) {
                    branch(shared, node.clone(), new, reservation)
                } else {
                    branch(shared, new, node.clone(), reservation)
                }
            })
            .transpose();
    }
    match &node.kind {
        NodeKind::Leaf(_) => after.map(|value| leaf(key, value, reservation)).transpose(),
        NodeKind::Branch { bit, left, right } => {
            // The immediate parent of a removed leaf collapses without allocating.
            let replacement = if key_bit(&key, *bit) {
                match replace_node(Some(right), key, after, reservation)? {
                    Some(new) => branch(*bit, left.clone(), new, reservation)?,
                    None => left.clone(),
                }
            } else {
                match replace_node(Some(left), key, after, reservation)? {
                    Some(new) => branch(*bit, new, right.clone(), reservation)?,
                    None => right.clone(),
                }
            };
            Ok(Some(replacement))
        }
    }
}
