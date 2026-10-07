//! Persistent depth32 wallet maps over source-selected immutable object roots.
//!
//! Every operation reads bounded paths, authenticates its G1 openings and returns
//! a new snapshot. Failed publication never changes the previous snapshot. Slot
//! allocation is monotonic through u32::MAX; removed slots are cleared and never
//! reused. Only the native custody manifest may make a returned snapshot current.

use super::*;

/// Native persistent indexed map with the exact protocol Poseidon root.
/// Serialized snapshots require source-selected custody; decoding alone is not authority.
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::PersistentIndexedMapV1")]
pub struct PersistentIndexedMapV1 {
    leaves: IndexRoot,
    nodes: IndexRoot,
    next_slot: u64,
    root: [u8; 32],
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::PersistentIndexedEntryV1")]
struct Entry {
    slot: u32,
    leaf: KagemushaWalletIndexedLeafV1,
}

fn ordered(mut key: [u8; 32]) -> [u8; 32] {
    key.reverse();
    key
}
fn location(height: usize, slot: u32) -> [u8; 32] {
    let mut key = [0; 32];
    key[0] = u8::try_from(height).expect("tree height <=32");
    key[1..5].copy_from_slice(&slot.to_be_bytes());
    key
}
fn node(left: &[u8; 32], right: &[u8; 32]) -> Result<[u8; 32], Error> {
    valid(kagemusha_wallet_indexed_node_v1(left, right))
}
fn key_valid(key: &[u8; 32]) -> Result<(), Error> {
    if *key == [0; 32] || !kagemusha_wallet_is_canonical_field_v1(key) {
        return Err(Error::Invalid("indexed map key"));
    }
    Ok(())
}

impl Default for PersistentIndexedMapV1 {
    fn default() -> Self {
        Self {
            leaves: IndexRoot::default(),
            nodes: IndexRoot::default(),
            next_slot: 1,
            root: kagemusha_wallet_empty_map_root_v1(),
        }
    }
}

impl PersistentIndexedMapV1 {
    /// Exact G1 root; snapshot custody remains mandatory.
    #[must_use]
    pub const fn root(&self) -> [u8; 32] {
        self.root
    }

    fn defaults() -> Result<[[u8; 32]; 33], Error> {
        let mut roots = [[0; 32]; 33];
        roots[0] = valid(KagemushaWalletIndexedLeafV1::SENTINEL.hash())?;
        for height in 0..32 {
            roots[height + 1] = node(
                &roots[height],
                &valid(kagemusha_wallet_indexed_empty_subtree_v1(height))?,
            )?;
        }
        Ok(roots)
    }
    fn hash(
        &self,
        store: &mut impl ObjectStore,
        height: usize,
        slot: u32,
        defaults: &[[u8; 32]; 33],
    ) -> Result<[u8; 32], Error> {
        match self.nodes.get(store, &location(height, slot))? {
            Some(bytes) => {
                let hash = bytes.try_into().map_err(|_| Error::WitnessLost("indexed node length"))?;
                if !kagemusha_wallet_is_canonical_field_v1(&hash) {
                    return Err(Error::WitnessLost("indexed node field"));
                }
                Ok(hash)
            }
            None if slot == 0 => Ok(defaults[height]),
            None => valid(kagemusha_wallet_indexed_empty_subtree_v1(height)),
        }
    }
    fn validate(&self, store: &mut impl ObjectStore) -> Result<[[u8; 32]; 33], Error> {
        if !(1..=1_u64 << 32).contains(&self.next_slot) {
            return Err(Error::WitnessLost("indexed allocation frontier"));
        }
        let defaults = Self::defaults()?;
        if self.hash(store, 32, 0, &defaults)? != self.root {
            return Err(Error::WitnessLost("indexed root"));
        }
        Ok(defaults)
    }
    fn decode_entry(&self, key: &[u8; 32], bytes: &[u8]) -> Result<Entry, Error> {
        let entry: Entry = archive::decode(bytes)?;
        valid(entry.leaf.validate())?;
        if ordered(entry.leaf.key) != *key
            || u64::from(entry.slot) >= self.next_slot
            || (entry.slot == 0) != (entry.leaf.key == [0; 32])
        {
            return Err(Error::WitnessLost("indexed leaf binding"));
        }
        Ok(entry)
    }
    fn predecessor(&self, store: &mut impl ObjectStore, key: &[u8; 32]) -> Result<Entry, Error> {
        match self.leaves.predecessor(store, &ordered(*key))? {
            Some((key, bytes)) => self.decode_entry(&key, &bytes),
            None => Ok(Entry { slot: 0, leaf: KagemushaWalletIndexedLeafV1::SENTINEL }),
        }
    }
    fn opening(
        &self,
        store: &mut impl ObjectStore,
        entry: &Entry,
        defaults: &[[u8; 32]; 33],
    ) -> Result<KagemushaWalletIndexedOpeningV1, Error> {
        let opening = self.slot_opening(store, entry.slot, defaults)?;
        if valid(opening.leaf_root(&entry.leaf))? != self.root {
            return Err(Error::WitnessLost("indexed opening root"));
        }
        Ok(opening)
    }
    fn slot_opening(
        &self,
        store: &mut impl ObjectStore,
        slot: u32,
        defaults: &[[u8; 32]; 33],
    ) -> Result<KagemushaWalletIndexedOpeningV1, Error> {
        let mut siblings = [[0; 32]; 32];
        let mut index = slot;
        for (height, sibling) in siblings.iter_mut().enumerate() {
            *sibling = self.hash(store, height, index ^ 1, defaults)?;
            index >>= 1;
        }
        Ok(KagemushaWalletIndexedOpeningV1 { slot, siblings })
    }
    fn set_leaf(
        &mut self,
        store: &mut impl ObjectStore,
        entry: &Entry,
        defaults: &[[u8; 32]; 33],
    ) -> Result<(), Error> {
        self.leaves = self.leaves.set(store, ordered(entry.leaf.key), &archive::encode(entry)?)?;
        self.write_slot(store, entry.slot, valid(entry.leaf.hash())?, defaults)
    }
    fn write_slot(
        &mut self,
        store: &mut impl ObjectStore,
        mut slot: u32,
        mut hash: [u8; 32],
        defaults: &[[u8; 32]; 33],
    ) -> Result<(), Error> {
        self.nodes = self.nodes.set(store, location(0, slot), &hash)?;
        for height in 0..32 {
            let sibling = self.hash(store, height, slot ^ 1, defaults)?;
            hash = if slot & 1 == 0 { node(&hash, &sibling)? } else { node(&sibling, &hash)? };
            slot >>= 1;
            self.nodes = self.nodes.set(store, location(height + 1, slot), &hash)?;
        }
        self.root = hash;
        Ok(())
    }

    /// Exact membership of a present key, or authenticated non-membership.
    /// Missing referenced archive data fails; it never means an absent map entry.
    ///
    /// # Errors
    /// Invalid key, malformed snapshot, unavailable storage or inconsistent opening.
    pub fn membership(
        &self,
        store: &mut impl ObjectStore,
        key: &[u8; 32],
    ) -> Result<Option<(KagemushaWalletIndexedLeafV1, KagemushaWalletIndexedOpeningV1)>, Error> {
        key_valid(key)?;
        let defaults = self.validate(store)?;
        let Some(bytes) = self.leaves.get(store, &ordered(*key))? else {
            self.non_membership(store, key)?;
            return Ok(None);
        };
        let entry = self.decode_entry(&ordered(*key), &bytes)?;
        Ok(Some((entry.leaf, self.opening(store, &entry, &defaults)?)))
    }

    /// Exact low-leaf witness for an absent key under this snapshot.
    ///
    /// # Errors
    /// Present/invalid key, unavailable storage or a changed low leaf or root.
    pub fn non_membership(
        &self,
        store: &mut impl ObjectStore,
        key: &[u8; 32],
    ) -> Result<(KagemushaWalletIndexedLeafV1, KagemushaWalletIndexedOpeningV1), Error> {
        key_valid(key)?;
        let defaults = self.validate(store)?;
        if self.leaves.get(store, &ordered(*key))?.is_some() {
            return Err(Error::Invalid("indexed key present"));
        }
        let low = self.predecessor(store, key)?;
        let opening = self.opening(store, &low, &defaults)?;
        valid(kagemusha_wallet_indexed_verify_non_membership_v1(&self.root, key, &low.leaf, &opening))?;
        Ok((low.leaf, opening))
    }

    /// Return the inserted snapshot and both exact G1 openings, allocating the next
    /// never-used slot. Publishing immutable nodes alone changes no wallet authority.
    ///
    /// # Errors
    /// Invalid/present key, invalid value, exhausted slots, changed empty-slot root,
    /// malformed snapshot or any uncertain storage operation.
    pub fn insert(
        &self,
        store: &mut impl ObjectStore,
        key: [u8; 32],
        value: [u8; 32],
    ) -> Result<(Self, KagemushaWalletIndexedInsertV1), Error> {
        key_valid(&value)?;
        let (low, low_opening) = self.non_membership(store, &key)?;
        let slot = u32::try_from(self.next_slot).map_err(|_| Error::Invalid("indexed tree full"))?;
        let defaults = Self::defaults()?;
        let mut next = self.clone();
        next.set_leaf(store, &Entry {
            slot: low_opening.slot,
            leaf: KagemushaWalletIndexedLeafV1 { next_key: key, ..low },
        }, &defaults)?;
        let slot_opening = next.slot_opening(store, slot, &defaults)?;
        if valid(slot_opening.empty_root())? != next.root {
            return Err(Error::WitnessLost("indexed insertion slot not empty"));
        }
        next.set_leaf(store, &Entry {
            slot,
            leaf: KagemushaWalletIndexedLeafV1 { key, value, next_key: low.next_key },
        }, &defaults)?;
        next.next_slot = self.next_slot.checked_add(1).ok_or(Error::Invalid("indexed tree full"))?;
        let witness = KagemushaWalletIndexedInsertV1 { low, low_opening, slot_opening };
        if valid(witness.verify(&self.root, &key, &value))? != next.root {
            return Err(Error::WitnessLost("indexed insertion root"));
        }
        Ok((next, witness))
    }

    /// Return the removed snapshot and exact G1 relink/clear witnesses. Slot
    /// allocation never decreases, including when the last live entry is removed.
    ///
    /// # Errors
    /// Missing/invalid key, changed predecessor, slot collision or storage failure.
    pub fn remove(
        &self,
        store: &mut impl ObjectStore,
        key: &[u8; 32],
    ) -> Result<(Self, KagemushaWalletIndexedRemoveV1), Error> {
        let (leaf, opening) = self.membership(store, key)?.ok_or(Error::Invalid("indexed key absent"))?;
        let defaults = Self::defaults()?;
        let predecessor = self.predecessor(store, key)?;
        let predecessor_opening = self.opening(store, &predecessor, &defaults)?;
        if predecessor.leaf.next_key != *key || predecessor.slot == opening.slot {
            return Err(Error::WitnessLost("indexed removal predecessor"));
        }
        let mut next = self.clone();
        next.set_leaf(store, &Entry {
            slot: predecessor.slot,
            leaf: KagemushaWalletIndexedLeafV1 { next_key: leaf.next_key, ..predecessor.leaf },
        }, &defaults)?;
        let leaf_opening = next.opening(store, &Entry { slot: opening.slot, leaf }, &defaults)?;
        next.leaves = next.leaves.remove(store, &ordered(*key))?;
        next.write_slot(store, opening.slot, [0; 32], &defaults)?;
        let witness = KagemushaWalletIndexedRemoveV1 {
            predecessor: predecessor.leaf,
            predecessor_opening,
            leaf,
            leaf_opening,
        };
        if valid(witness.verify(&self.root, key))? != next.root {
            return Err(Error::WitnessLost("indexed removal root"));
        }
        Ok((next, witness))
    }
}

#[cfg(test)]
mod tests;
