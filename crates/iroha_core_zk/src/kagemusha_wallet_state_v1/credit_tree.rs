//! Persistent credit-digest IMT with bounded path updates and the exact G1 Poseidon root.

use super::*;

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::CreditTree")]
pub(super) struct CreditTree {
    leaves: IndexRoot,
    nodes: IndexRoot,
    next_slot: u64,
    pub root: [u8; 32],
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::CreditEntry")]
struct Entry {
    slot: u32,
    key: [u8; 32],
    value: [u8; 32],
    next_key: [u8; 32],
    payment_digest: [u8; 32],
    burned: bool,
}
impl Entry {
    fn leaf(&self) -> KagemushaWalletIndexedLeafV1 {
        KagemushaWalletIndexedLeafV1 {
            key: self.key,
            value: self.value,
            next_key: self.next_key,
        }
    }
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
impl Default for CreditTree {
    fn default() -> Self {
        Self {
            leaves: IndexRoot::default(),
            nodes: IndexRoot::default(),
            next_slot: 1,
            root: kagemusha_wallet_empty_map_root_v1(),
        }
    }
}
impl CreditTree {
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
            Some(bytes) => bytes
                .try_into()
                .map_err(|_| Error::WitnessLost("credit node length")),
            None if slot == 0 => Ok(defaults[height]),
            None => valid(kagemusha_wallet_indexed_empty_subtree_v1(height)),
        }
    }
    fn set_leaf(
        &mut self,
        store: &mut impl ObjectStore,
        entry: &Entry,
        defaults: &[[u8; 32]; 33],
    ) -> Result<(), Error> {
        self.leaves = self
            .leaves
            .set(store, ordered(entry.key), &archive::encode(entry)?)?;
        let mut slot = entry.slot;
        let mut hash = valid(entry.leaf().hash())?;
        self.nodes = self.nodes.set(store, location(0, slot), &hash)?;
        for height in 0..32 {
            let sibling = self.hash(store, height, slot ^ 1, defaults)?;
            hash = if slot & 1 == 0 {
                node(&hash, &sibling)?
            } else {
                node(&sibling, &hash)?
            };
            slot >>= 1;
            self.nodes = self.nodes.set(store, location(height + 1, slot), &hash)?;
        }
        self.root = hash;
        Ok(())
    }
    pub fn record(
        &mut self,
        store: &mut impl ObjectStore,
        credit: &KagemushaWalletCreditDigestLeafV1,
    ) -> Result<(), Error> {
        valid(credit.field_items())?;
        let key = ordered(credit.credit_id);
        if self.leaves.get(store, &key)?.is_some() {
            return Ok(());
        }
        let mut previous: Entry = match self.leaves.predecessor(store, &key)? {
            Some((_, bytes)) => archive::decode(&bytes)?,
            None => Entry {
                slot: 0,
                key: [0; 32],
                value: [0; 32],
                next_key: [0; 32],
                payment_digest: [0; 32],
                burned: false,
            },
        };
        if !previous.leaf().brackets(&credit.credit_id) {
            return Err(Error::WitnessLost("credit predecessor"));
        }
        let entry = Entry {
            slot: u32::try_from(self.next_slot).map_err(|_| Error::Invalid("credit tree full"))?,
            key: credit.credit_id,
            value: valid(credit.leaf_value())?,
            next_key: previous.next_key,
            payment_digest: credit.payment_digest,
            burned: credit.burned,
        };
        previous.next_key = credit.credit_id;
        let defaults = Self::defaults()?;
        self.set_leaf(store, &previous, &defaults)?;
        self.set_leaf(store, &entry, &defaults)?;
        self.next_slot += 1;
        Ok(())
    }
    pub fn opening(
        &self,
        store: &mut impl ObjectStore,
        credit_id: &[u8; 32],
    ) -> Result<
        (
            KagemushaWalletCreditDigestLeafV1,
            KagemushaWalletIndexedLeafV1,
            KagemushaWalletIndexedOpeningV1,
        ),
        Error,
    > {
        let entry: Entry = archive::decode(
            &self
                .leaves
                .get(store, &ordered(*credit_id))?
                .ok_or(Error::FoldRequired)?,
        )?;
        if entry.key != *credit_id {
            return Err(Error::WitnessLost("credit leaf key"));
        }
        let defaults = Self::defaults()?;
        let mut siblings = [[0; 32]; 32];
        let mut slot = entry.slot;
        for (height, sibling) in siblings.iter_mut().enumerate() {
            *sibling = self.hash(store, height, slot ^ 1, &defaults)?;
            slot >>= 1;
        }
        let leaf = entry.leaf();
        let opening = KagemushaWalletIndexedOpeningV1 {
            slot: entry.slot,
            siblings,
        };
        if valid(opening.root_over(&valid(leaf.hash())?))? != self.root {
            return Err(Error::WitnessLost("credit opening root"));
        }
        Ok((
            KagemushaWalletCreditDigestLeafV1 {
                credit_id: entry.key,
                payment_digest: entry.payment_digest,
                burned: entry.burned,
            },
            leaf,
            opening,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistent_tree_matches_native_insert_order_openings_and_first_identity() {
        let mut store = super::super::tests::MemoryArchive::new();
        let mut tree = CreditTree::default();
        let mut native = KagemushaWalletIndexedTreeV1::new();
        for number in [19u8, 7, 25, 2, 31, 11, 3, 9] {
            let mut id = [0; 32];
            id[0] = number;
            let mut payment = [0; 32];
            payment[0] = number + 42;
            let credit = KagemushaWalletCreditDigestLeafV1 {
                credit_id: id,
                payment_digest: payment,
                burned: number % 2 == 0,
            };
            tree.record(&mut store, &credit).expect("persistent insert");
            credit.record(&mut native).expect("native insert");
            assert_eq!(tree.root, native.root());
            let (_, leaf, opening) = tree.opening(&mut store, &id).expect("persistent opening");
            assert_eq!(
                (leaf, opening),
                native.membership(&id).expect("native opening")
            );
            let first = tree.root;
            let changed = KagemushaWalletCreditDigestLeafV1 {
                burned: !credit.burned,
                payment_digest: id,
                ..credit
            };
            tree.record(&mut store, &changed)
                .expect("first identity remains");
            assert_eq!(tree.root, first);
            assert_eq!(tree.opening(&mut store, &id).expect("opening").0, credit);
        }
    }
}
