//! Native references of the trees the step relations open (wire record
//! `specs/kagemusha_wallet_wire_v1.md` sections 3.2 and 3.3; proposal
//! sections 3 and 7): the blacklist gap tree, the quota-window tree and the
//! depth-32 indexed Merkle tree of the wallet maps, with their domains,
//! leaves, nodes, openings and small builders for witnesses and tests.
//!
//! These are the in-circuit values of `crate::control_circuit`, computed
//! with [`hash_with_domain`] over the σ field. The G1 data model owns the
//! wallet's own trees; the shared vectors of
//! `fixtures/kagemusha/wallet_v1_vectors.json` pin both
//! (`tests/digest_parity.rs`).
//!
//! # Which σ opens which tree
//!
//! - `sigma_send` with the blacklist control: one gap opening of the
//!   Request's receiver account digest in the payer's blacklist gap tree
//!   ([`BlacklistTree`], depth 16), against the head-committed
//!   `blacklist_root`.
//! - `sigma_recv` with the blacklist control: one gap opening of the
//!   Request's payer account digest in the receiver's gap tree.
//! - `sigma_send` with the quota control: openings of the quota-window tree
//!   ([`QuotaWindowTree`], depth 6) and value updates or insertions in the
//!   quota-usage map ([`IndexedTree`], depth 32), against the head-committed
//!   `quota_windows_root` and `quota_usage_root`.
//!
//! The lineage relation opens every other map (consumed-credit,
//! pending-outgoing, load/redeem recovery, fee-claim) and the credit-digest
//! tree; no σ opens them (proposal section 3).

use std::collections::BTreeMap;

use iroha_pasta::poseidon::{PoseidonField, hash_with_domain};
use iroha_plonk_gadgets::statement::digest_fields;

/// Blacklist gap-leaf domain `kgwblkl1` (G1
/// `KAGEMUSHA_WALLET_BLACKLIST_LEAF_DOMAIN_V1`).
pub const BLACKLIST_LEAF_DOMAIN: u64 = u64::from_le_bytes(*b"kgwblkl1");
/// Blacklist node domain `kgwblkn1` (G1
/// `KAGEMUSHA_WALLET_BLACKLIST_NODE_DOMAIN_V1`).
pub const BLACKLIST_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwblkn1");
/// Depth of the blacklist gap tree (65,536 gap leaves).
pub const BLACKLIST_DEPTH: usize = 16;
/// The lowest blacklist sentinel `00..00`.
pub const BLACKLIST_SENTINEL_LOW: [u8; 32] = [0; 32];
/// The highest blacklist sentinel `FF..FF`.
pub const BLACKLIST_SENTINEL_HIGH: [u8; 32] = [0xff; 32];
/// The most blacklist entries (G1 `KAGEMUSHA_WALLET_BLACKLIST_ENTRIES_MAX_V1`).
pub const BLACKLIST_ENTRIES_MAX: usize = 65_535;

/// Quota-window leaf domain `kgwqwin1` (G1
/// `KAGEMUSHA_WALLET_QUOTA_WINDOW_DOMAIN_V1`).
pub const QUOTA_WINDOW_DOMAIN: u64 = u64::from_le_bytes(*b"kgwqwin1");
/// Quota-window node domain `kgwqwnd1` (G1
/// `KAGEMUSHA_WALLET_QUOTA_NODE_DOMAIN_V1`).
pub const QUOTA_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwqwnd1");
/// Depth of the quota-window tree.
pub const QUOTA_DEPTH: usize = 6;
/// Window slots of a quota share (G1 `KAGEMUSHA_WALLET_QUOTA_WINDOWS_MAX_V1`).
pub const QUOTA_SLOTS: usize = 1 << QUOTA_DEPTH;
/// The Daily window kind tag.
pub const WINDOW_DAILY: u8 = 1;
/// The Monthly window kind tag.
pub const WINDOW_MONTHLY: u8 = 2;
/// The window kind tags, in tree order.
pub const WINDOW_KINDS: [u8; 2] = [WINDOW_DAILY, WINDOW_MONTHLY];

/// Indexed-tree leaf domain `kgwimlf1` (G1
/// `KAGEMUSHA_WALLET_INDEXED_LEAF_DOMAIN_V1`).
pub const INDEXED_LEAF_DOMAIN: u64 = u64::from_le_bytes(*b"kgwimlf1");
/// Indexed-tree node domain `kgwimnd1` (G1
/// `KAGEMUSHA_WALLET_INDEXED_NODE_DOMAIN_V1`).
pub const INDEXED_NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwimnd1");
/// Depth of every indexed map tree.
pub const INDEXED_DEPTH: usize = 32;
/// Quota-usage value domain `kgwquse1` (G1
/// `KAGEMUSHA_WALLET_QUOTA_USAGE_VALUE_DOMAIN_V1`).
pub const QUOTA_USAGE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwquse1");

/// Whether `left < right` in the blacklist's limb order (owner answer A4):
/// `int(a) = hi * 2^128 + lo`, the 32 bytes read as one little-endian
/// integer.
#[must_use]
pub fn limb_less(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter().rev().cmp(right.iter().rev()) == core::cmp::Ordering::Less
}

/// Whether `left < right` as integers in `[0, p)`.
#[must_use]
pub fn field_less<F: PoseidonField>(left: &F, right: &F) -> bool {
    let left = left.to_canonical_limbs();
    let right = right.to_canonical_limbs();
    left.iter().rev().cmp(right.iter().rev()) == core::cmp::Ordering::Less
}

/// The root over `node` at the leaf position `index` of a binary tree whose
/// nodes are `P(node_domain, [left, right])`: bit `h` of `index` puts the
/// running node on the right at height `h`.
#[must_use]
pub fn path_root<F: PoseidonField>(node_domain: u64, node: F, index: u64, siblings: &[F]) -> F {
    let mut current = node;
    for (height, sibling) in siblings.iter().enumerate() {
        current = if (index >> height) & 1 == 1 {
            hash_with_domain(node_domain, &[*sibling, current])
        } else {
            hash_with_domain(node_domain, &[current, *sibling])
        };
    }
    current
}

// ---------------------------------------------------------------------------
// Blacklist gap tree (wire record section 3.3)
// ---------------------------------------------------------------------------

/// The gap leaf `P(kgwblkl1, limbs(lower) || limbs(upper))`.
#[must_use]
pub fn blacklist_leaf<F: PoseidonField>(lower: &[u8; 32], upper: &[u8; 32]) -> F {
    let [lower_lo, lower_hi] = digest_fields::<F>(lower);
    let [upper_lo, upper_hi] = digest_fields::<F>(upper);
    hash_with_domain(
        BLACKLIST_LEAF_DOMAIN,
        &[lower_lo, lower_hi, upper_lo, upper_hi],
    )
}

/// A blacklist non-membership opening: the gap leaf `(lower, upper)` at
/// `leaf_index` and its 16 siblings (a local witness, never transmitted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlacklistGap<F> {
    /// The gap leaf index.
    pub leaf_index: u32,
    /// The lower sentinel or entry of the gap.
    pub lower: [u8; 32],
    /// The upper entry or sentinel of the gap.
    pub upper: [u8; 32],
    /// The siblings, height 0 first.
    pub siblings: [F; BLACKLIST_DEPTH],
}

impl<F: PoseidonField> BlacklistGap<F> {
    /// The all-zero opening of a relation whose list is not enforced.
    #[must_use]
    pub fn unused() -> Self {
        Self {
            leaf_index: 0,
            lower: [0; 32],
            upper: [0; 32],
            siblings: [F::ZERO; BLACKLIST_DEPTH],
        }
    }

    /// The gap leaf.
    #[must_use]
    pub fn leaf(&self) -> F {
        blacklist_leaf(&self.lower, &self.upper)
    }

    /// The root the opening recomputes.
    #[must_use]
    pub fn root(&self) -> F {
        path_root(
            BLACKLIST_NODE_DOMAIN,
            self.leaf(),
            u64::from(self.leaf_index),
            &self.siblings,
        )
    }

    /// Whether `account` lies strictly inside the gap in limb order.
    #[must_use]
    pub fn brackets(&self, account: &[u8; 32]) -> bool {
        limb_less(&self.lower, account) && limb_less(account, &self.upper)
    }

    /// Whether the opening proves `account` absent from the tree `root`.
    #[must_use]
    pub fn proves_absent(&self, root: &F, account: &[u8; 32]) -> bool {
        self.brackets(account) && self.root() == *root
    }
}

/// A blacklist gap tree over sorted entries (the G1
/// `kagemusha_wallet_blacklist_root_v1` construction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlacklistTree {
    entries: Vec<[u8; 32]>,
}

impl BlacklistTree {
    /// The tree of `entries`, sorted here into limb order (`None` for a
    /// duplicate, a sentinel value or too many entries).
    #[must_use]
    pub fn new(mut entries: Vec<[u8; 32]>) -> Option<Self> {
        entries.sort_by(|left, right| left.iter().rev().cmp(right.iter().rev()));
        let duplicate = entries.windows(2).any(|pair| pair[0] == pair[1]);
        let sentinel = entries
            .iter()
            .any(|entry| *entry == BLACKLIST_SENTINEL_LOW || *entry == BLACKLIST_SENTINEL_HIGH);
        (!duplicate && !sentinel && entries.len() <= BLACKLIST_ENTRIES_MAX)
            .then_some(Self { entries })
    }

    /// The entries in limb order.
    #[must_use]
    pub fn entries(&self) -> &[[u8; 32]] {
        &self.entries
    }

    /// Every level (gap leaves first, the root last) and the padding root of
    /// every height.
    fn levels<F: PoseidonField>(&self) -> (Vec<Vec<F>>, [F; BLACKLIST_DEPTH + 1]) {
        let mut padding = [F::ZERO; BLACKLIST_DEPTH + 1];
        padding[0] = blacklist_leaf(&BLACKLIST_SENTINEL_HIGH, &BLACKLIST_SENTINEL_HIGH);
        for height in 0..BLACKLIST_DEPTH {
            padding[height + 1] =
                hash_with_domain(BLACKLIST_NODE_DOMAIN, &[padding[height], padding[height]]);
        }
        let mut level = Vec::with_capacity(self.entries.len() + 1);
        let mut lower = &BLACKLIST_SENTINEL_LOW;
        for entry in &self.entries {
            level.push(blacklist_leaf(lower, entry));
            lower = entry;
        }
        level.push(blacklist_leaf(lower, &BLACKLIST_SENTINEL_HIGH));
        let mut levels = Vec::with_capacity(BLACKLIST_DEPTH + 1);
        for pad in padding.iter().take(BLACKLIST_DEPTH) {
            let next = level
                .chunks(2)
                .map(|pair| {
                    hash_with_domain(
                        BLACKLIST_NODE_DOMAIN,
                        &[pair[0], *pair.get(1).unwrap_or(pad)],
                    )
                })
                .collect();
            levels.push(level);
            level = next;
        }
        levels.push(level);
        (levels, padding)
    }

    /// The gap-tree root.
    #[must_use]
    pub fn root<F: PoseidonField>(&self) -> F {
        let (levels, _) = self.levels::<F>();
        levels
            .last()
            .and_then(|root| root.first())
            .copied()
            .unwrap_or(F::ZERO)
    }

    /// The gap opening of an absent `account` (`None` when it is listed or a
    /// sentinel).
    #[must_use]
    pub fn gap<F: PoseidonField>(&self, account: &[u8; 32]) -> Option<BlacklistGap<F>> {
        if *account == BLACKLIST_SENTINEL_LOW || *account == BLACKLIST_SENTINEL_HIGH {
            return None;
        }
        let index = self
            .entries
            .partition_point(|entry| limb_less(entry, account));
        let upper = self
            .entries
            .get(index)
            .copied()
            .unwrap_or(BLACKLIST_SENTINEL_HIGH);
        if upper == *account {
            return None;
        }
        let lower = index
            .checked_sub(1)
            .and_then(|previous| self.entries.get(previous))
            .copied()
            .unwrap_or(BLACKLIST_SENTINEL_LOW);
        let (levels, padding) = self.levels::<F>();
        let mut siblings = [F::ZERO; BLACKLIST_DEPTH];
        let mut position = index;
        for ((sibling, level), pad) in siblings.iter_mut().zip(&levels).zip(&padding) {
            *sibling = level.get(position ^ 1).copied().unwrap_or(*pad);
            position >>= 1;
        }
        Some(BlacklistGap {
            leaf_index: u32::try_from(index).ok()?,
            lower,
            upper,
            siblings,
        })
    }
}

// ---------------------------------------------------------------------------
// Quota-window tree (wire record section 3.3)
// ---------------------------------------------------------------------------

/// One quota-window slot: `kind` Daily 1 or Monthly 2, the half-open
/// `[start_ms, end_ms)` and the gross limit; the empty slot is all zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuotaWindow {
    /// The window kind tag (0 for an empty slot).
    pub kind: u8,
    /// The inclusive start in Unix milliseconds.
    pub start_ms: u64,
    /// The exclusive end in Unix milliseconds.
    pub end_ms: u64,
    /// The gross sending limit.
    pub limit: u128,
}

impl QuotaWindow {
    /// The empty slot `(0, 0, 0, 0)`.
    pub const EMPTY: Self = Self {
        kind: 0,
        start_ms: 0,
        end_ms: 0,
        limit: 0,
    };

    /// The window leaf `P(kgwqwin1, [kind, start, end, limit])`.
    #[must_use]
    pub fn leaf<F: PoseidonField>(&self) -> F {
        hash_with_domain(
            QUOTA_WINDOW_DOMAIN,
            &[
                F::from(u64::from(self.kind)),
                F::from(self.start_ms),
                F::from(self.end_ms),
                F::from_u128(self.limit),
            ],
        )
    }

    /// Whether a Send over `[lower, upper]` touches the window: `start <=
    /// upper` and `lower < end` (wire record section 3.3, Time).
    #[must_use]
    pub const fn touches(&self, lower: u64, upper: u64) -> bool {
        self.start_ms <= upper && lower < self.end_ms
    }

    /// The quota-usage map key `kind * 2^128 + start`.
    #[must_use]
    pub fn usage_key<F: PoseidonField>(&self) -> F {
        usage_key(self.kind, self.start_ms)
    }

    /// The quota-usage value `P(kgwquse1, [kind, start, end, used])` of this
    /// window with `used` (a field value: the circuit's sum).
    #[must_use]
    pub fn usage_value<F: PoseidonField>(&self, used: F) -> F {
        hash_with_domain(
            QUOTA_USAGE_DOMAIN,
            &[
                F::from(u64::from(self.kind)),
                F::from(self.start_ms),
                F::from(self.end_ms),
                used,
            ],
        )
    }
}

/// The quota-usage map key `kind * 2^128 + start_ms`.
#[must_use]
pub fn usage_key<F: PoseidonField>(kind: u8, start_ms: u64) -> F {
    F::from(u64::from(kind)) * F::from_u128(1 << 64) * F::from_u128(1 << 64) + F::from(start_ms)
}

/// A quota-window tree: the windows first, sorted by `(kind, start)`, then
/// empty slots (the G1 `kagemusha_wallet_quota_windows_root_v1`
/// construction).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuotaWindowTree {
    slots: Vec<QuotaWindow>,
}

impl QuotaWindowTree {
    /// The tree of `windows` (`None` for more than 64 windows, an empty or
    /// unknown kind, or windows out of `(kind, start)` order).
    #[must_use]
    pub fn new(windows: &[QuotaWindow]) -> Option<Self> {
        if windows.len() > QUOTA_SLOTS
            || windows
                .iter()
                .any(|window| !WINDOW_KINDS.contains(&window.kind))
            || windows
                .windows(2)
                .any(|pair| (pair[0].kind, pair[0].start_ms) >= (pair[1].kind, pair[1].start_ms))
        {
            return None;
        }
        let mut slots = windows.to_vec();
        slots.resize(QUOTA_SLOTS, QuotaWindow::EMPTY);
        Some(Self { slots })
    }

    /// The window (or empty slot) at `slot`.
    #[must_use]
    pub fn slot(&self, slot: usize) -> QuotaWindow {
        self.slots.get(slot).copied().unwrap_or(QuotaWindow::EMPTY)
    }

    /// Every level, leaves first.
    fn levels<F: PoseidonField>(&self) -> Vec<Vec<F>> {
        let mut level: Vec<F> = self.slots.iter().map(QuotaWindow::leaf).collect();
        let mut levels = Vec::with_capacity(QUOTA_DEPTH + 1);
        while level.len() > 1 {
            let next = level
                .chunks_exact(2)
                .map(|pair| hash_with_domain(QUOTA_NODE_DOMAIN, &[pair[0], pair[1]]))
                .collect();
            levels.push(level);
            level = next;
        }
        levels.push(level);
        levels
    }

    /// The windows root.
    #[must_use]
    pub fn root<F: PoseidonField>(&self) -> F {
        self.levels::<F>()
            .last()
            .and_then(|root| root.first())
            .copied()
            .unwrap_or(F::ZERO)
    }

    /// The siblings of `slot`, height 0 first.
    #[must_use]
    pub fn siblings<F: PoseidonField>(&self, slot: usize) -> [F; QUOTA_DEPTH] {
        let levels = self.levels::<F>();
        let mut siblings = [F::ZERO; QUOTA_DEPTH];
        let mut position = slot;
        for (sibling, level) in siblings.iter_mut().zip(&levels) {
            *sibling = level.get(position ^ 1).copied().unwrap_or(F::ZERO);
            position >>= 1;
        }
        siblings
    }
}

// ---------------------------------------------------------------------------
// Indexed Merkle tree (wire record section 3.2)
// ---------------------------------------------------------------------------

/// One indexed-tree leaf `(key, value, next_key)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndexedLeaf<F> {
    /// The key (zero only for the sentinel).
    pub key: F,
    /// The map value (zero only for the sentinel).
    pub value: F,
    /// The next larger key present, or zero for the largest.
    pub next_key: F,
}

impl<F: PoseidonField> IndexedLeaf<F> {
    /// The zero sentinel leaf of slot 0.
    #[must_use]
    pub fn sentinel() -> Self {
        Self {
            key: F::ZERO,
            value: F::ZERO,
            next_key: F::ZERO,
        }
    }

    /// The leaf hash `P(kgwimlf1, [key, value, next_key])`.
    #[must_use]
    pub fn hash(&self) -> F {
        hash_with_domain(INDEXED_LEAF_DOMAIN, &[self.key, self.value, self.next_key])
    }

    /// Whether the leaf is the low leaf of an absent `key`: `self.key < key`
    /// and (`next_key = 0` or `key < next_key`).
    #[must_use]
    pub fn brackets(&self, key: &F) -> bool {
        field_less(&self.key, key)
            && (bool::from(self.next_key.is_zero()) || field_less(key, &self.next_key))
    }
}

/// The empty subtree root of every height (height 0: the empty slot `0`).
#[must_use]
pub fn indexed_empty<F: PoseidonField>() -> [F; INDEXED_DEPTH + 1] {
    let mut empty = [F::ZERO; INDEXED_DEPTH + 1];
    for height in 0..INDEXED_DEPTH {
        empty[height + 1] = hash_with_domain(INDEXED_NODE_DOMAIN, &[empty[height], empty[height]]);
    }
    empty
}

/// A sparse depth-32 indexed Merkle tree (the G1
/// `KagemushaWalletIndexedTreeV1` construction): the zero sentinel in slot
/// 0, insertions at the next free slot, removal never used by a σ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedTree<F> {
    slots: BTreeMap<u32, IndexedLeaf<F>>,
    nodes: Vec<BTreeMap<u64, F>>,
    empty: [F; INDEXED_DEPTH + 1],
    next_free: u32,
}

impl<F: PoseidonField> Default for IndexedTree<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: PoseidonField> IndexedTree<F> {
    /// The empty tree: the zero sentinel leaf in slot 0.
    #[must_use]
    pub fn new() -> Self {
        let mut tree = Self {
            slots: BTreeMap::new(),
            nodes: vec![BTreeMap::new(); INDEXED_DEPTH + 1],
            empty: indexed_empty(),
            next_free: 1,
        };
        tree.write(0, Some(IndexedLeaf::sentinel()));
        tree
    }

    /// The root.
    #[must_use]
    pub fn root(&self) -> F {
        self.node(INDEXED_DEPTH, 0)
    }

    /// The next free slot.
    #[must_use]
    pub const fn next_free(&self) -> u32 {
        self.next_free
    }

    fn node(&self, height: usize, index: u64) -> F {
        self.nodes[height]
            .get(&index)
            .copied()
            .unwrap_or(self.empty[height])
    }

    fn write(&mut self, slot: u32, leaf: Option<IndexedLeaf<F>>) {
        let mut index = u64::from(slot);
        if let Some(leaf) = leaf {
            self.slots.insert(slot, leaf);
            self.nodes[0].insert(index, leaf.hash());
        } else {
            self.slots.remove(&slot);
            self.nodes[0].remove(&index);
        }
        for height in 0..INDEXED_DEPTH {
            let left = self.node(height, index & !1);
            let right = self.node(height, index | 1);
            index >>= 1;
            let parent = hash_with_domain(INDEXED_NODE_DOMAIN, &[left, right]);
            if parent == self.empty[height + 1] {
                self.nodes[height + 1].remove(&index);
            } else {
                self.nodes[height + 1].insert(index, parent);
            }
        }
    }

    /// The siblings of `slot`, height 0 first.
    #[must_use]
    pub fn siblings(&self, slot: u32) -> [F; INDEXED_DEPTH] {
        let mut siblings = [F::ZERO; INDEXED_DEPTH];
        let mut index = u64::from(slot);
        for (height, sibling) in siblings.iter_mut().enumerate() {
            *sibling = self.node(height, index ^ 1);
            index >>= 1;
        }
        siblings
    }

    /// The slot and leaf of a present `key`.
    #[must_use]
    pub fn find(&self, key: &F) -> Option<(u32, IndexedLeaf<F>)> {
        self.slots
            .iter()
            .find(|(_, leaf)| leaf.key == *key && !bool::from(key.is_zero()))
            .map(|(slot, leaf)| (*slot, *leaf))
    }

    /// The slot and leaf of the low leaf of an absent nonzero `key`.
    #[must_use]
    pub fn low(&self, key: &F) -> Option<(u32, IndexedLeaf<F>)> {
        self.slots
            .iter()
            .find(|(_, leaf)| leaf.brackets(key))
            .map(|(slot, leaf)| (*slot, *leaf))
    }

    /// The value of `key`, if present.
    #[must_use]
    pub fn get(&self, key: &F) -> Option<F> {
        self.find(key).map(|(_, leaf)| leaf.value)
    }

    /// Updates a present `key` or inserts an absent one with `value`, and
    /// returns the witness of the operation (`None` for a zero key or a
    /// full tree).
    #[must_use]
    pub fn upsert(&mut self, key: F, value: F) -> Option<IndexedUpsert<F>> {
        if bool::from(key.is_zero()) {
            return None;
        }
        if let Some((slot, leaf)) = self.find(&key) {
            let leaf_siblings = self.siblings(slot);
            self.write(slot, Some(IndexedLeaf { value, ..leaf }));
            // The no-op empty-slot opening: the highest slot, never written
            // by a wallet that has not filled its tree.
            let empty_slot = u32::MAX;
            return Some(IndexedUpsert {
                insert: false,
                leaf,
                leaf_slot: slot,
                leaf_siblings,
                slot: empty_slot,
                slot_siblings: self.siblings(empty_slot),
            });
        }
        let (low_slot, low) = self.low(&key)?;
        let slot = self.next_free;
        if slot == u32::MAX {
            return None;
        }
        let leaf_siblings = self.siblings(low_slot);
        self.write(
            low_slot,
            Some(IndexedLeaf {
                next_key: key,
                ..low
            }),
        );
        let slot_siblings = self.siblings(slot);
        self.write(
            slot,
            Some(IndexedLeaf {
                key,
                value,
                next_key: low.next_key,
            }),
        );
        self.next_free = slot + 1;
        Some(IndexedUpsert {
            insert: true,
            leaf: low,
            leaf_slot: low_slot,
            leaf_siblings,
            slot,
            slot_siblings,
        })
    }
}

/// The witness of one indexed-tree update or insertion, in the unified
/// form the quota charge opens: the key's leaf (update) or its low leaf
/// (insertion) with its opening against the old root, then an empty slot
/// opened against the root after the first write (the written slot of an
/// insertion; any empty slot, written back empty, for an update).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedUpsert<F> {
    /// Whether the key was absent (an insertion).
    pub insert: bool,
    /// The key's leaf (update) or the low leaf (insertion), before the
    /// operation.
    pub leaf: IndexedLeaf<F>,
    /// The slot of `leaf`.
    pub leaf_slot: u32,
    /// The siblings of `leaf_slot` against the old root.
    pub leaf_siblings: [F; INDEXED_DEPTH],
    /// The written slot (insertion) or the empty no-op slot (update).
    pub slot: u32,
    /// The siblings of `slot` against the root after the first write.
    pub slot_siblings: [F; INDEXED_DEPTH],
}

impl<F: PoseidonField> IndexedUpsert<F> {
    /// The all-zero witness of a charge that is not taken.
    #[must_use]
    pub fn unused() -> Self {
        Self {
            insert: false,
            leaf: IndexedLeaf::sentinel(),
            leaf_slot: 0,
            leaf_siblings: [F::ZERO; INDEXED_DEPTH],
            slot: 0,
            slot_siblings: [F::ZERO; INDEXED_DEPTH],
        }
    }
}

#[cfg(test)]
mod tests {
    use ff::{Field, PrimeField};
    use iroha_pasta::{Fp, Fq};

    use super::*;

    #[test]
    fn domains_are_the_g1_ones() {
        assert_eq!(BLACKLIST_LEAF_DOMAIN.to_le_bytes(), *b"kgwblkl1");
        assert_eq!(BLACKLIST_NODE_DOMAIN.to_le_bytes(), *b"kgwblkn1");
        assert_eq!(QUOTA_WINDOW_DOMAIN.to_le_bytes(), *b"kgwqwin1");
        assert_eq!(QUOTA_NODE_DOMAIN.to_le_bytes(), *b"kgwqwnd1");
        assert_eq!(INDEXED_LEAF_DOMAIN.to_le_bytes(), *b"kgwimlf1");
        assert_eq!(INDEXED_NODE_DOMAIN.to_le_bytes(), *b"kgwimnd1");
        assert_eq!(QUOTA_USAGE_DOMAIN.to_le_bytes(), *b"kgwquse1");
    }

    #[test]
    fn limb_order_reads_the_digest_as_one_little_endian_integer() {
        let mut low_hi = [0_u8; 32];
        low_hi[31] = 1;
        let mut high_lo = [0_u8; 32];
        high_lo[0] = 0xff;
        // Byte order says high_lo > low_hi; limb order the reverse.
        assert!(limb_less(&high_lo, &low_hi));
        assert!(!limb_less(&low_hi, &high_lo));
        assert!(!limb_less(&low_hi, &low_hi));
        assert!(limb_less(&BLACKLIST_SENTINEL_LOW, &high_lo));
        assert!(limb_less(&low_hi, &BLACKLIST_SENTINEL_HIGH));
        assert!(field_less(&Fp::from(3_u64), &Fp::from(4_u64)));
        assert!(!field_less(&-Fp::ONE, &Fp::from(4_u64)));
    }

    #[test]
    fn gap_openings_reach_the_root_and_bracket_only_absent_accounts() {
        let entries = vec![[0x30; 32], [0x10; 32], [0x20; 32]];
        let tree = BlacklistTree::new(entries).expect("tree");
        assert_eq!(tree.entries()[0], [0x10; 32]);
        let root: Fp = tree.root();
        for account in [[0x01; 32], [0x15; 32], [0x25; 32], [0x40; 32]] {
            let gap: BlacklistGap<Fp> = tree.gap(&account).expect("absent");
            assert!(gap.proves_absent(&root, &account), "{account:?}");
            assert!(!gap.proves_absent(&(root + Fp::ONE), &account));
        }
        assert!(tree.gap::<Fp>(&[0x20; 32]).is_none());
        assert!(tree.gap::<Fp>(&BLACKLIST_SENTINEL_HIGH).is_none());
        // A gap does not bracket its own bounds.
        let gap: BlacklistGap<Fq> = tree.gap(&[0x15; 32]).expect("absent");
        assert!(!gap.brackets(&gap.lower) && !gap.brackets(&gap.upper));
        assert!(BlacklistTree::new(vec![[1; 32], [1; 32]]).is_none());
        assert!(BlacklistTree::new(vec![BLACKLIST_SENTINEL_HIGH]).is_none());
        // The empty list is one gap from sentinel to sentinel.
        let empty = BlacklistTree::new(Vec::new()).expect("empty");
        let gap: BlacklistGap<Fp> = empty.gap(&[0x77; 32]).expect("absent");
        assert_eq!(gap.leaf_index, 0);
        assert!(gap.proves_absent(&empty.root(), &[0x77; 32]));
    }

    #[test]
    fn window_trees_open_every_slot() {
        let windows = [
            QuotaWindow {
                kind: WINDOW_DAILY,
                start_ms: 10,
                end_ms: 20,
                limit: 5,
            },
            QuotaWindow {
                kind: WINDOW_MONTHLY,
                start_ms: 0,
                end_ms: 100,
                limit: 50,
            },
        ];
        let tree = QuotaWindowTree::new(&windows).expect("tree");
        let root: Fp = tree.root();
        for slot in [0, 1, 2, 63] {
            let leaf = tree.slot(slot).leaf::<Fp>();
            let siblings = tree.siblings::<Fp>(slot);
            assert_eq!(
                path_root(QUOTA_NODE_DOMAIN, leaf, slot as u64, &siblings),
                root
            );
        }
        assert_eq!(tree.slot(5), QuotaWindow::EMPTY);
        assert!(QuotaWindowTree::new(&[windows[1], windows[0]]).is_none());
        assert!(windows[0].touches(19, 30) && !windows[0].touches(20, 30));
        assert!(windows[0].touches(0, 10) && !windows[0].touches(0, 9));
        assert_eq!(
            windows[1].usage_key::<Fp>(),
            Fp::from_u128(1 << 64).square().double() + Fp::ZERO
        );
    }

    #[test]
    fn indexed_upserts_verify_against_the_roots() {
        let mut tree = IndexedTree::<Fp>::new();
        assert_eq!(tree.root(), {
            let empty = indexed_empty::<Fp>();
            path_root(
                INDEXED_NODE_DOMAIN,
                IndexedLeaf::<Fp>::sentinel().hash(),
                0,
                &empty[..INDEXED_DEPTH],
            )
        });
        for (key, value) in [(5_u64, 50_u64), (3, 30), (9, 90), (5, 55)] {
            let old = tree.root();
            let key = Fp::from(key);
            let value = Fp::from(value);
            let upsert = tree.upsert(key, value).expect("upsert");
            let leaf_root = path_root(
                INDEXED_NODE_DOMAIN,
                upsert.leaf.hash(),
                u64::from(upsert.leaf_slot),
                &upsert.leaf_siblings,
            );
            assert_eq!(leaf_root, old);
            let written = if upsert.insert {
                assert!(upsert.leaf.brackets(&key));
                IndexedLeaf {
                    next_key: key,
                    ..upsert.leaf
                }
            } else {
                assert_eq!(upsert.leaf.key, key);
                IndexedLeaf {
                    value,
                    ..upsert.leaf
                }
            };
            let middle = path_root(
                INDEXED_NODE_DOMAIN,
                written.hash(),
                u64::from(upsert.leaf_slot),
                &upsert.leaf_siblings,
            );
            let slot = u64::from(upsert.slot);
            assert_eq!(
                path_root(INDEXED_NODE_DOMAIN, Fp::ZERO, slot, &upsert.slot_siblings),
                middle
            );
            let content = if upsert.insert {
                IndexedLeaf {
                    key,
                    value,
                    next_key: upsert.leaf.next_key,
                }
                .hash()
            } else {
                Fp::ZERO
            };
            assert_eq!(
                path_root(INDEXED_NODE_DOMAIN, content, slot, &upsert.slot_siblings),
                tree.root()
            );
            assert_eq!(tree.get(&key), Some(value));
        }
        assert_eq!(tree.next_free(), 4);
        assert!(tree.upsert(Fp::ZERO, Fp::ONE).is_none());
        assert_eq!(IndexedUpsert::<Fq>::unused().leaf, IndexedLeaf::sentinel());
    }
}
