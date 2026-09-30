//! Checked fixed geometry and original prepaid custody for ordered Merkle levels.

use super::{
    DigestEntry, Hash, KEY_DOMAIN, MAX_NORITO_TREE_ENTRIES, NoritoKeyRangeError, branch_hash,
    digest_frame, leaf_hash, pad_hash,
};
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, AllocationReservation, ChargedBuffer,
    InsufficientReservation,
};
use std::alloc::Layout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Geometry {
    leaves: usize,
    levels: usize,
    bytes: usize,
}

impl Geometry {
    fn for_entries(entries: usize) -> Result<Self, NoritoKeyRangeError> {
        if entries > MAX_NORITO_TREE_ENTRIES {
            return Err(NoritoKeyRangeError::Capacity);
        }
        let (leaves, levels, nodes) = if entries == 0 {
            (0, 0, 0)
        } else {
            let leaves = entries
                .checked_next_power_of_two()
                .ok_or(NoritoKeyRangeError::Capacity)?;
            let nodes = leaves
                .checked_mul(2)
                .and_then(|count| count.checked_sub(1))
                .ok_or(NoritoKeyRangeError::Capacity)?;
            (leaves, leaves.ilog2() as usize + 1, nodes)
        };
        let outer = Layout::array::<ChargedBuffer<Hash>>(levels)
            .map_err(|_| NoritoKeyRangeError::Admission(AllocationRefusal::DemandOverflow))?;
        let nodes = Layout::array::<Hash>(nodes)
            .map_err(|_| NoritoKeyRangeError::Admission(AllocationRefusal::DemandOverflow))?;
        let bytes =
            outer
                .size()
                .checked_add(nodes.size())
                .ok_or(NoritoKeyRangeError::Admission(
                    AllocationRefusal::DemandOverflow,
                ))?;
        Ok(Self {
            leaves,
            levels,
            bytes,
        })
    }
}

/// Fund every concrete level and the outer owner from the same original pool.
pub(super) fn build(
    entries: &[DigestEntry],
    budget: &AllocationBudget,
) -> Result<ChargedBuffer<ChargedBuffer<Hash>>, NoritoKeyRangeError> {
    let geometry = Geometry::for_entries(entries.len())?;
    let mut reservation = budget
        .try_reserve_bytes(geometry.bytes)
        .map_err(NoritoKeyRangeError::Admission)?;
    build_prepaid(entries, geometry, &mut reservation)
}

fn build_prepaid(
    entries: &[DigestEntry],
    geometry: Geometry,
    reservation: &mut AllocationReservation,
) -> Result<ChargedBuffer<ChargedBuffer<Hash>>, NoritoKeyRangeError> {
    // Reject a short parent before allocating even the outer owner. No second
    // pool admission occurs after this point, even while the original pool is fully occupied.
    if reservation.remaining_bytes() < geometry.bytes {
        return Err(NoritoKeyRangeError::PrepaidCapacity(
            InsufficientReservation {
                requested_bytes: geometry.bytes,
                remaining_bytes: reservation.remaining_bytes(),
            },
        ));
    }
    let initial_credit = reservation.remaining_bytes();
    let outer_layout = Layout::array::<ChargedBuffer<Hash>>(geometry.levels)
        .map_err(|_| NoritoKeyRangeError::Admission(AllocationRefusal::DemandOverflow))?;
    let outer_charge = reservation
        .try_split(outer_layout)
        .map_err(NoritoKeyRangeError::PrepaidCapacity)?;
    // The exact charge already funds this layout. On allocator refusal the returned
    // charge is abandoned here; no physical backing escaped and the remainder stays owned.
    let mut levels = ChargedBuffer::try_from_charge(geometry.levels, outer_charge)
        .map_err(|(_charge, _error)| NoritoKeyRangeError::Allocation)?;
    if geometry.levels == 0 {
        return Ok(levels);
    }
    let leaf_layout = Layout::array::<Hash>(geometry.leaves)
        .map_err(|_| NoritoKeyRangeError::Admission(AllocationRefusal::DemandOverflow))?;
    let leaf_charge = reservation
        .try_split(leaf_layout)
        .map_err(NoritoKeyRangeError::PrepaidCapacity)?;
    let mut leaves = ChargedBuffer::try_from_charge(geometry.leaves, leaf_charge)
        .map_err(|(_charge, _error)| NoritoKeyRangeError::Allocation)?;
    for (index, entry) in entries.iter().enumerate() {
        leaves
            .try_push(leaf_hash(
                u32::try_from(index).expect("bounded entry index fits u32"),
                digest_frame(KEY_DOMAIN, entry.key.as_slice()),
                entry.value_digest,
            ))
            .map_err(|_| NoritoKeyRangeError::Capacity)?;
    }
    for index in entries.len()..geometry.leaves {
        leaves
            .try_push(pad_hash(
                u32::try_from(index).expect("bounded pad index fits u32"),
            ))
            .map_err(|_| NoritoKeyRangeError::Capacity)?;
    }
    levels
        .try_push(leaves)
        .map_err(|_| NoritoKeyRangeError::Capacity)?;
    let mut level = 0_u8;
    while levels.as_slice().len() < geometry.levels {
        let current = levels.as_slice().last().expect("nonempty tree has a level");
        let parent_count = current.as_slice().len() / 2;
        let parent_layout = Layout::array::<Hash>(parent_count)
            .map_err(|_| NoritoKeyRangeError::Admission(AllocationRefusal::DemandOverflow))?;
        let parent_charge = reservation
            .try_split(parent_layout)
            .map_err(NoritoKeyRangeError::PrepaidCapacity)?;
        let mut parents = ChargedBuffer::try_from_charge(parent_count, parent_charge)
            .map_err(|(_charge, _error)| NoritoKeyRangeError::Allocation)?;
        for pair in current.as_slice().chunks_exact(2) {
            parents
                .try_push(branch_hash(level, pair[0], pair[1]))
                .map_err(|_| NoritoKeyRangeError::Capacity)?;
        }
        levels
            .try_push(parents)
            .map_err(|_| NoritoKeyRangeError::Capacity)?;
        level += 1;
    }
    debug_assert_eq!(
        initial_credit - reservation.remaining_bytes(),
        geometry.bytes
    );
    Ok(levels)
}

#[cfg(test)]
#[path = "level_funding_tests.rs"]
mod tests;
