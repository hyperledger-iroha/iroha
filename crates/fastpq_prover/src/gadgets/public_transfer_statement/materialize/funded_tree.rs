//! One prepaid flat touched-tree kernel and move-only canonical witness custody.

use std::alloc::Layout;

use iroha_allocation::{AllocationCharge, AllocationRefusal, ChargedBuffer};

use super::*;

#[derive(Clone, Copy)]
struct Node {
    path: u32,
    hash: Hash,
}

fn overflow() -> crate::Error {
    AllocationRefusal::DemandOverflow.into()
}

fn add_layout<T>(bytes: &mut usize, count: usize) -> Result<()> {
    let layout = Layout::array::<T>(count).map_err(|_| overflow())?;
    *bytes = bytes.checked_add(layout.size()).ok_or_else(overflow)?;
    Ok(())
}

fn counts(limits: TransferSmtBuildLimits, updates: usize, unique_keys: usize) -> Result<usize> {
    check_limit("max_transfer_smt_updates", updates, limits.max_updates)?;
    check_limit("max_transfer_smt_keys", unique_keys, limits.max_unique_keys)?;
    check_limit(
        "max_transfer_smt_nodes",
        unique_keys,
        limits.max_retained_nodes,
    )?;
    if !updates.is_multiple_of(2) || (updates == 0) != (unique_keys == 0) {
        return Err(invariant("SMT public table cardinality is inconsistent"));
    }
    let siblings = updates
        .checked_mul(HEIGHT)
        .ok_or_else(|| invariant("SMT sibling count overflows"))?;
    check_limit(
        "max_transfer_smt_siblings",
        siblings,
        limits.max_sibling_hashes,
    )?;
    Ok(siblings)
}

pub(super) fn allocation_bytes(
    limits: TransferSmtBuildLimits,
    updates: usize,
    unique_keys: usize,
) -> Result<usize> {
    counts(limits, updates, unique_keys)?;
    let nodes = unique_keys
        .checked_mul(HEIGHT + 1)
        .ok_or_else(overflow)?
        .min(limits.max_retained_nodes);
    let mut bytes = 0;
    add_layout::<u32>(&mut bytes, unique_keys)?;
    add_layout::<Node>(&mut bytes, nodes)?;
    add_layout::<bool>(&mut bytes, unique_keys)?;
    add_layout::<bool>(&mut bytes, updates)?;
    add_layout::<[TransferSmtWitness; 2]>(&mut bytes, updates / 2)?;
    add_layout::<AllocationCharge>(&mut bytes, updates.checked_mul(2).ok_or_else(overflow)?)?;
    // Every update keeps exactly four little-endian path bytes and 32 hashes.
    // Validate each concrete layout before multiplying its size by the count.
    let mut per_update = 0;
    add_layout::<u8>(&mut per_update, 4)?;
    add_layout::<[u8; 32]>(&mut per_update, HEIGHT)?;
    bytes
        .checked_add(per_update.checked_mul(updates).ok_or_else(overflow)?)
        .ok_or_else(overflow)
}

fn preflight<T: CheckedUpdateTable + ?Sized>(
    prepared: &T,
    limits: TransferSmtBuildLimits,
    updates: usize,
    reservation: &mut AllocationReservation,
) -> Result<(TransferSmtBuildWork, ChargedBuffer<u32>)> {
    let unique_keys = prepared.keys().len();
    let sibling_hashes = counts(limits, updates, unique_keys)?;
    if updates != prepared.row_count() {
        return Err(invariant("SMT public table cardinality is inconsistent"));
    }
    let mut paths = ChargedBuffer::from_reservation(unique_keys, reservation)?;
    for key in prepared.keys() {
        paths.push_reserved(key.path);
    }
    paths.as_mut_slice().sort_unstable();
    if paths.as_slice().windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invariant("SMT public key paths are not unique"));
    }
    let mut retained_nodes = 0_usize;
    for level in 0..=HEIGHT {
        let mut previous = None;
        for path in paths.as_slice() {
            let parent = u64::from(*path) >> level;
            if previous != Some(parent) {
                retained_nodes = checked_add(retained_nodes, 1)?;
                check_limit(
                    "max_transfer_smt_nodes",
                    retained_nodes,
                    limits.max_retained_nodes,
                )?;
                previous = Some(parent);
            }
        }
    }
    let node_hashes = checked_add(retained_nodes - unique_keys, sibling_hashes)?;
    check_limit(
        "max_transfer_smt_node_hashes",
        node_hashes,
        limits.max_node_hashes,
    )?;
    Ok((
        TransferSmtBuildWork {
            updates,
            unique_keys,
            retained_nodes,
            sibling_hashes,
            node_hashes,
        },
        paths,
    ))
}

fn flags(count: usize, reservation: &mut AllocationReservation) -> Result<ChargedBuffer<bool>> {
    let mut flags = ChargedBuffer::from_reservation(count, reservation)?;
    for _ in 0..count {
        flags.push_reserved(false);
    }
    Ok(flags)
}

// Canonical payloads are declared before their original nested allocation ledger.
// Rust drops fields in declaration order: all primitive Vec backing is physically
// deallocated before its credits refund. Only immutable slices leave this owner.
pub(super) struct WitnessPairs {
    pairs: ChargedBuffer<[TransferSmtWitness; 2]>,
    nested: ChargedBuffer<AllocationCharge>,
}

impl WitnessPairs {
    fn new(pair_count: usize, reservation: &mut AllocationReservation) -> Result<Self> {
        let charges = pair_count.checked_mul(4).ok_or_else(overflow)?;
        Ok(Self {
            pairs: ChargedBuffer::from_reservation(pair_count, reservation)?,
            nested: ChargedBuffer::from_reservation(charges, reservation)?,
        })
    }

    pub(super) fn as_slice(&self) -> &[[TransferSmtWitness; 2]] {
        self.pairs.as_slice()
    }

    fn push(&mut self, first: FundedWitness, second: FundedWitness) {
        // Both owners still retain their credits if either invariant panics.
        // After these checks, fixed-capacity pushes cannot allocate or refuse.
        assert!(self.pairs.as_slice().len() < self.pairs.capacity());
        assert!(self.nested.capacity() - self.nested.as_slice().len() >= 4);
        let FundedWitness {
            witness: first,
            charges: first_charges,
        } = first;
        let FundedWitness {
            witness: second,
            charges: second_charges,
        } = second;
        self.pairs.push_reserved([first, second]);
        for charge in first_charges.into_iter().chain(second_charges) {
            self.nested.push_reserved(charge);
        }
    }
}

struct FundedWitness {
    witness: TransferSmtWitness,
    charges: [AllocationCharge; 2],
}

impl FundedWitness {
    #[allow(unsafe_code)]
    fn new(
        roots: ([u8; 32], [u8; 32]),
        path: ChargedBuffer<u8>,
        siblings: ChargedBuffer<[u8; 32]>,
    ) -> Self {
        // SAFETY: these two fixed buffers contain only primitive values. The
        // extraction, canonical constructor and immediate move into this owner
        // cannot allocate or fail. No mutation/extraction API exposes their Vecs.
        // Both allocations drop through `witness` before `charges`; the only
        // onward move is WitnessPairs::push, preserving the identical custody.
        let (path, path_charge) = unsafe { path.into_allocation_parts() };
        let (siblings, sibling_charge) = unsafe { siblings.into_allocation_parts() };
        Self {
            witness: TransferSmtWitness::new(roots.0, roots.1, path, siblings),
            charges: [path_charge, sibling_charge],
        }
    }
}

pub(super) fn derive<T: CheckedUpdateTable + ?Sized>(
    prepared: &T,
    limits: TransferSmtBuildLimits,
    budget: &AllocationBudget,
    reservation: &mut AllocationReservation,
) -> Result<DerivedTransferSmtWitnesses> {
    if !reservation.belongs_to(budget) {
        return Err(crate::Error::AllocationForeignPool);
    }
    let updates = prepared
        .pair_count()
        .checked_mul(2)
        .ok_or_else(|| invariant("SMT update count overflows"))?;
    let demand = allocation_bytes(limits, updates, prepared.keys().len())?;
    let mut reservation = reservation.try_partition_bytes(demand)?;
    let (work, paths) = preflight(prepared, limits, updates, &mut reservation)?;
    if work.updates == 0 {
        let public_inputs = prepared.public_inputs();
        if public_inputs.old_root != public_inputs.new_root {
            return Err(invariant("empty SMT update sequence changes its root"));
        }
        return Ok(DerivedTransferSmtWitnesses {
            roots: (public_inputs.old_root, public_inputs.new_root),
            pairs: WitnessPairs::new(0, &mut reservation)?,
            work,
        });
    }
    let mut tree = Tree::new(paths.as_slice(), work.retained_nodes, &mut reservation)?;
    let mut initial = flags(work.unique_keys, &mut reservation)?;
    let mut rows_seen = flags(work.updates, &mut reservation)?;
    let pair_count = work.updates / 2;
    for ordinal in 0..pair_count {
        let pair = prepared
            .pair(ordinal)
            .ok_or_else(|| invariant("SMT pair index is invalid"))?;
        if pair.occurrence[2] as usize != ordinal {
            return Err(invariant("SMT pair occurrence order is inconsistent"));
        }
        for (leg, index) in pair.row_indices.into_iter().enumerate() {
            let seen = rows_seen
                .as_mut_slice()
                .get_mut(index)
                .ok_or_else(|| invariant("SMT row index is invalid"))?;
            let row = prepared
                .row(index)
                .ok_or_else(|| invariant("SMT row index is invalid"))?;
            let key = prepared
                .keys()
                .get(row.key_index)
                .ok_or_else(|| invariant("SMT key index is invalid"))?;
            if std::mem::replace(seen, true)
                || row.leg != leg
                || row.update != pair.updates[leg]
                || row.update.path != key.path
                || row.occurrence != pair.occurrence
            {
                return Err(invariant(
                    "SMT pair ports differ from their exact public rows",
                ));
            }
            if !initial.as_slice()[row.key_index] {
                tree.set(0, key.path, digest(row.update.old_leaf)?);
                initial.as_mut_slice()[row.key_index] = true;
            }
        }
    }
    if rows_seen.as_slice().iter().any(|seen| !seen) {
        return Err(invariant("SMT row lacks a chronological occurrence"));
    }
    if initial.as_slice().iter().any(|seen| !seen) {
        return Err(invariant("SMT key lacks a first public occurrence"));
    }
    tree.seed();
    let old_root = tree.root().into();
    let mut pairs = WitnessPairs::new(pair_count, &mut reservation)?;
    for ordinal in 0..pair_count {
        let pair = prepared
            .pair(ordinal)
            .ok_or_else(|| invariant("SMT pair index is invalid"))?;
        let first = tree.update(pair.updates[0], &mut reservation)?;
        let second = tree.update(pair.updates[1], &mut reservation)?;
        pairs.push(first, second);
    }
    if tree.hashes != work.node_hashes || tree.nodes.as_slice().len() != work.retained_nodes {
        return Err(invariant("SMT construction work differs from preflight"));
    }
    Ok(DerivedTransferSmtWitnesses {
        roots: (old_root, tree.root().into()),
        pairs,
        work,
    })
}

struct Tree {
    nodes: ChargedBuffer<Node>,
    // start[level]..start[level + 1] is one sorted level, including level 32.
    start: [usize; HEIGHT + 2],
    pads: [Hash; HEIGHT + 1],
    hashes: usize,
}

impl Tree {
    fn new(
        paths: &[u32],
        retained_nodes: usize,
        reservation: &mut AllocationReservation,
    ) -> Result<Self> {
        let pads = core::array::from_fn(padding);
        let mut nodes = ChargedBuffer::from_reservation(retained_nodes, reservation)?;
        let mut start = [0; HEIGHT + 2];
        for level in 0..=HEIGHT {
            start[level] = nodes.as_slice().len();
            let mut previous = None;
            for path in paths {
                // The 64-bit shift preserves the level-32 root at path zero.
                let parent =
                    u32::try_from(u64::from(*path) >> level).expect("a shifted u32 path fits u32");
                if previous != Some(parent) {
                    nodes.push_reserved(Node {
                        path: parent,
                        hash: pads[level],
                    });
                    previous = Some(parent);
                }
            }
        }
        start[HEIGHT + 1] = nodes.as_slice().len();
        Ok(Self {
            nodes,
            start,
            pads,
            hashes: 0,
        })
    }

    fn index(&self, level: usize, path: u32) -> Option<usize> {
        self.nodes.as_slice()[self.start[level]..self.start[level + 1]]
            .binary_search_by_key(&path, |node| node.path)
            .ok()
            .map(|offset| self.start[level] + offset)
    }

    fn get(&self, level: usize, path: u32) -> Option<Hash> {
        self.index(level, path)
            .map(|index| self.nodes.as_slice()[index].hash)
    }

    fn set(&mut self, level: usize, path: u32, hash: Hash) {
        let index = self
            .index(level, path)
            .expect("preflight retained every touched ancestor");
        self.nodes.as_mut_slice()[index].hash = hash;
    }

    fn node(&mut self, level: usize, parent: u32) -> Hash {
        let left = self.get(level, parent << 1).unwrap_or(self.pads[level]);
        let right = self
            .get(level, (parent << 1) | 1)
            .unwrap_or(self.pads[level]);
        let hash = Hash::new_from_chunks(&[NODE_DOMAIN, left.as_ref(), right.as_ref()]);
        self.hashes += 1; // The checked preflight bounds every seed/update hash.
        hash
    }

    fn seed(&mut self) {
        for level in 0..HEIGHT {
            for index in self.start[level + 1]..self.start[level + 2] {
                let parent = self.nodes.as_slice()[index].path;
                let hash = self.node(level, parent);
                self.nodes.as_mut_slice()[index].hash = hash;
            }
        }
    }

    fn root(&self) -> Hash {
        self.get(HEIGHT, 0).unwrap_or(self.pads[HEIGHT])
    }

    fn update(
        &mut self,
        update: PublicUpdate,
        reservation: &mut AllocationReservation,
    ) -> Result<FundedWitness> {
        let before = digest(update.old_leaf)?;
        let after = digest(update.new_leaf)?;
        if self.get(0, update.path) != Some(before) {
            return Err(invariant(
                "SMT chronological pre-leaf does not match current state",
            ));
        }
        // Allocate both original backings before changing any tree node. On any
        // refusal the existing tree and all earlier canonical owners stay funded.
        let mut path_bytes = ChargedBuffer::from_reservation(4, reservation)?;
        for byte in update.path.to_le_bytes() {
            path_bytes.push_reserved(byte);
        }
        let mut siblings = ChargedBuffer::from_reservation(HEIGHT, reservation)?;
        let root_before = self.root().into();
        let mut path = update.path;
        self.set(0, path, after);
        for level in 0..HEIGHT {
            siblings.push_reserved(self.get(level, path ^ 1).unwrap_or(self.pads[level]).into());
            let parent = path >> 1;
            let hash = self.node(level, parent);
            self.set(level + 1, parent, hash);
            path = parent;
        }
        Ok(FundedWitness::new(
            (root_before, self.root().into()),
            path_bytes,
            siblings,
        ))
    }
}

#[cfg(test)]
mod tests;
