//! σ-field Poseidon values of the wallet: the `P` and `P_bytes` hashes, their domains and the
//! depth-256 sparse Merkle tree of the credit-digest root and every state map (§§3, 3.2, 4.1,
//! 5.1; owner answers Q1, Q2, Q7, Q9 and Q10 of 2026-10-05).
//!
//! `P(d, items)` is the RP57 Poseidon `iroha_pasta::poseidon::hash_with_domain` with domain word
//! `d` over Pasta `Fp`, the Vesta scalar field in which the step proofs σ are proved. Its value is
//! one canonical field element, carried as its 32-byte little-endian encoding (`< p`). Element
//! lists follow one rule: an integer, tag or mask is one element; a 32-byte SHA-256 digest or
//! identifier is two `u128` limbs, low half first; a `P` value (commitment, chain, root, nonce,
//! `credit_id`, `proof_digest`, Payment digest) is one element.
//!
//! `P_bytes(d, b) = P(d, [len(b)] || c_0 || ... || c_(m-1))` hashes a byte string: `len(b)` is
//! the byte length as one element, `m = ceil(len(b) / 31)` and `c_i` is bytes `31i .. 31i + 30`
//! of `b` read as a little-endian integer, the last chunk zero-filled. Every chunk is below
//! `2^248 < p`, so the packing is injective for a fixed length and the length element separates
//! lengths.
//!
//! The sparse Merkle tree has depth 256 and is keyed by the canonical 32-byte little-endian
//! encoding of a field element read as an unsigned integer: the leaf index is the key. The node
//! at height `h + 1` with index `i` has the children `2i` (left) and `2i + 1` (right) at height
//! `h`, so bit `h` of the key selects the child at height `h`. An empty leaf is
//! `P(kgwsmte1, [])`; an empty subtree of height `h + 1` is the node over two empty subtrees of
//! height `h`; a node is `P(kgwsmtn1, [left, right])`. An opening of a key carries the presence
//! bitmap of its non-default siblings (bit `h` of byte `h / 8`, least significant first, for the
//! sibling at height `h`) and exactly those siblings, root-ward.

use std::{collections::BTreeMap, sync::OnceLock};

use iroha_pasta::{Fp, PastaField, poseidon::hash_with_domain};

use super::{WalletResult, digest::kagemusha_wallet_is_canonical_field_v1, invalid_v1};

#[cfg(test)]
#[path = "poseidon_tests.rs"]
pub(super) mod poseidon_tests;

const fn domain_v1(ascii: [u8; 8]) -> u64 {
    u64::from_le_bytes(ascii)
}

/// Poseidon domain of the state commitment `P(core elements || rest digest)` (§3).
pub const KAGEMUSHA_WALLET_CORE_DOMAIN_V1: u64 = domain_v1(*b"kgwcore1");
/// Poseidon domain of the rest digest (§3).
pub const KAGEMUSHA_WALLET_REST_DOMAIN_V1: u64 = domain_v1(*b"kgwrest1");
/// Poseidon domain of the σ public statement digest (§3.2).
pub const KAGEMUSHA_WALLET_STATEMENT_DOMAIN_V1: u64 = domain_v1(*b"kgwstmt1");
/// Poseidon domain of `credit_id` over the 24 Request body elements (§5.1, owner answer Q1).
pub const KAGEMUSHA_WALLET_CREDIT_DOMAIN_V1: u64 = domain_v1(*b"kgwcrdt1");
/// Poseidon domain of one `send_chain` append (§3).
pub const KAGEMUSHA_WALLET_SEND_CHAIN_DOMAIN_V1: u64 = domain_v1(*b"kgwschn1");
/// Poseidon domain of one `recv_chain` append (§3).
pub const KAGEMUSHA_WALLET_RECV_CHAIN_DOMAIN_V1: u64 = domain_v1(*b"kgwrchn1");
/// Poseidon domain of consumed-credit leaves (permanent map).
pub const KAGEMUSHA_WALLET_CONSUMED_CREDIT_LEAF_DOMAIN_V1: u64 = domain_v1(*b"kgwccrd1");
/// Poseidon domain of pending-outgoing leaves.
pub const KAGEMUSHA_WALLET_PENDING_OUTGOING_LEAF_DOMAIN_V1: u64 = domain_v1(*b"kgwpout1");
/// Poseidon domain of load leaves of the load/redeem recovery map.
pub const KAGEMUSHA_WALLET_LOAD_RECOVERY_LEAF_DOMAIN_V1: u64 = domain_v1(*b"kgwload1");
/// Poseidon domain of redeem leaves of the load/redeem recovery map.
pub const KAGEMUSHA_WALLET_REDEEM_RECOVERY_LEAF_DOMAIN_V1: u64 = domain_v1(*b"kgwrdm_1");
/// Poseidon domain of fee-claim leaves.
pub const KAGEMUSHA_WALLET_FEE_CLAIM_LEAF_DOMAIN_V1: u64 = domain_v1(*b"kgwfee_1");
/// Poseidon domain of quota-usage leaves.
pub const KAGEMUSHA_WALLET_QUOTA_USAGE_LEAF_DOMAIN_V1: u64 = domain_v1(*b"kgwquse1");
/// Poseidon domain of lineage-level credit-digest leaves (§3).
pub const KAGEMUSHA_WALLET_CREDIT_DIGEST_LEAF_DOMAIN_V1: u64 = domain_v1(*b"kgwcdig1");
/// Poseidon domain of the empty sparse-tree leaf `P(kgwsmte1, [])`.
pub const KAGEMUSHA_WALLET_SPARSE_EMPTY_DOMAIN_V1: u64 = domain_v1(*b"kgwsmte1");
/// Poseidon domain of a sparse-tree node `P(kgwsmtn1, [left, right])`.
pub const KAGEMUSHA_WALLET_SPARSE_NODE_DOMAIN_V1: u64 = domain_v1(*b"kgwsmtn1");
/// Poseidon domain of a blacklist gap leaf (§7).
pub const KAGEMUSHA_WALLET_BLACKLIST_LEAF_DOMAIN_V1: u64 = domain_v1(*b"kgwblkl1");
/// Poseidon domain of a blacklist tree node (§7).
pub const KAGEMUSHA_WALLET_BLACKLIST_NODE_DOMAIN_V1: u64 = domain_v1(*b"kgwblkn1");
/// Poseidon domain of a quota window leaf (§7).
pub const KAGEMUSHA_WALLET_QUOTA_WINDOW_DOMAIN_V1: u64 = domain_v1(*b"kgwqwin1");
/// Poseidon domain of a quota windows tree node (§7).
pub const KAGEMUSHA_WALLET_QUOTA_NODE_DOMAIN_V1: u64 = domain_v1(*b"kgwqwnd1");
/// `P_bytes` domain of `proof_digest` over `LE32 len(Ω) || Ω || LE32 len(σ) || σ` (Send, Unload,
/// Retiring; §4.1).
pub const KAGEMUSHA_WALLET_PROOF_DOMAIN_V1: u64 = domain_v1(*b"kgwprf_1");
/// `P_bytes` domain of the distinct σ-only `proof_digest` over `LE32 len(σ) || σ` (§4.1).
pub const KAGEMUSHA_WALLET_STEP_PROOF_DOMAIN_V1: u64 = domain_v1(*b"kgwstep1");
/// `P_bytes` domain of the Payment digest over the `payment` transcript (§5.1).
pub const KAGEMUSHA_WALLET_PAYMENT_DOMAIN_V1: u64 = domain_v1(*b"kgwpay_1");

/// Every Poseidon domain of the wallet by use, in table order.
pub const KAGEMUSHA_WALLET_POSEIDON_DOMAINS_V1: [(&str, u64); 22] = [
    ("core", KAGEMUSHA_WALLET_CORE_DOMAIN_V1),
    ("rest", KAGEMUSHA_WALLET_REST_DOMAIN_V1),
    ("statement", KAGEMUSHA_WALLET_STATEMENT_DOMAIN_V1),
    ("credit_id", KAGEMUSHA_WALLET_CREDIT_DOMAIN_V1),
    ("send_chain", KAGEMUSHA_WALLET_SEND_CHAIN_DOMAIN_V1),
    ("recv_chain", KAGEMUSHA_WALLET_RECV_CHAIN_DOMAIN_V1),
    (
        "consumed_credit_leaf",
        KAGEMUSHA_WALLET_CONSUMED_CREDIT_LEAF_DOMAIN_V1,
    ),
    (
        "pending_outgoing_leaf",
        KAGEMUSHA_WALLET_PENDING_OUTGOING_LEAF_DOMAIN_V1,
    ),
    (
        "load_recovery_leaf",
        KAGEMUSHA_WALLET_LOAD_RECOVERY_LEAF_DOMAIN_V1,
    ),
    (
        "redeem_recovery_leaf",
        KAGEMUSHA_WALLET_REDEEM_RECOVERY_LEAF_DOMAIN_V1,
    ),
    ("fee_claim_leaf", KAGEMUSHA_WALLET_FEE_CLAIM_LEAF_DOMAIN_V1),
    (
        "quota_usage_leaf",
        KAGEMUSHA_WALLET_QUOTA_USAGE_LEAF_DOMAIN_V1,
    ),
    (
        "credit_digest_leaf",
        KAGEMUSHA_WALLET_CREDIT_DIGEST_LEAF_DOMAIN_V1,
    ),
    ("sparse_empty_leaf", KAGEMUSHA_WALLET_SPARSE_EMPTY_DOMAIN_V1),
    ("sparse_node", KAGEMUSHA_WALLET_SPARSE_NODE_DOMAIN_V1),
    ("blacklist_leaf", KAGEMUSHA_WALLET_BLACKLIST_LEAF_DOMAIN_V1),
    ("blacklist_node", KAGEMUSHA_WALLET_BLACKLIST_NODE_DOMAIN_V1),
    ("quota_window_leaf", KAGEMUSHA_WALLET_QUOTA_WINDOW_DOMAIN_V1),
    ("quota_node", KAGEMUSHA_WALLET_QUOTA_NODE_DOMAIN_V1),
    ("proof_digest", KAGEMUSHA_WALLET_PROOF_DOMAIN_V1),
    ("step_proof_digest", KAGEMUSHA_WALLET_STEP_PROOF_DOMAIN_V1),
    ("payment_digest", KAGEMUSHA_WALLET_PAYMENT_DOMAIN_V1),
];

/// Bytes of one `P_bytes` chunk: 31, so every chunk is a canonical field element.
pub const KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1: usize = 31;
/// Depth of the sparse Merkle tree: one level per bit of a 256-bit key.
pub const KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1: usize = 256;

// ---------------------------------------------------------------------------------------
// Field conversions and `P`
// ---------------------------------------------------------------------------------------

fn limbs_of_v1(value: &[u8; 32]) -> [u64; 4] {
    let mut limbs = [0_u64; 4];
    for (limb, chunk) in limbs.iter_mut().zip(value.chunks_exact(8)) {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(chunk);
        *limb = u64::from_le_bytes(bytes);
    }
    limbs
}

fn bytes_of_limbs_v1(limbs: [u64; 4]) -> [u8; 32] {
    let mut value = [0_u8; 32];
    for (chunk, limb) in value.chunks_exact_mut(8).zip(limbs) {
        chunk.copy_from_slice(&limb.to_le_bytes());
    }
    value
}

/// The field element of a canonical encoding, or `None` for a value `>= p`.
fn field_of_v1(value: &[u8; 32]) -> Option<Fp> {
    Option::from(Fp::from_canonical_limbs(limbs_of_v1(value)))
}

/// Canonical encoding of a field element.
fn bytes_of_field_v1(value: &Fp) -> [u8; 32] {
    bytes_of_limbs_v1(value.to_canonical_limbs())
}

/// `P(domain, items)` over elements that are canonical by construction: integers, limbs and
/// field values already validated by their owner.
///
/// Every caller builds its items from integers below `2^128`, chunks below `2^248` or field
/// values it validated; the reduction never applies to such an item.
pub(super) fn poseidon_items_v1(domain: u64, items: &[[u8; 32]]) -> [u8; 32] {
    let inputs: Vec<Fp> = items
        .iter()
        .map(|item| {
            debug_assert!(kagemusha_wallet_is_canonical_field_v1(item));
            Fp::from_raw_reduced(limbs_of_v1(item))
        })
        .collect();
    bytes_of_field_v1(&hash_with_domain(domain, &inputs))
}

/// `P(domain, items)`: the RP57 Poseidon hash of canonical σ-field elements (§3).
///
/// # Errors
///
/// Rejects an item that is not a canonical encoding (`>= p`).
pub fn kagemusha_wallet_poseidon_v1(domain: u64, items: &[[u8; 32]]) -> WalletResult<[u8; 32]> {
    let inputs = items
        .iter()
        .map(|item| field_of_v1(item).ok_or_else(|| invalid_v1("poseidon.item")))
        .collect::<WalletResult<Vec<Fp>>>()?;
    Ok(bytes_of_field_v1(&hash_with_domain(domain, &inputs)))
}

/// The `P_bytes` element list of `bytes`: the byte length as one element, then the 31-byte
/// little-endian chunks, the last one zero-filled.
#[must_use]
pub fn kagemusha_wallet_packed_bytes_v1(bytes: &[u8]) -> Vec<[u8; 32]> {
    let chunks = bytes.len().div_ceil(KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1);
    let mut items = Vec::with_capacity(chunks.saturating_add(1));
    // `usize` is at most 64 bits on every admitted target, so the length is exact.
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let mut length = [0_u8; 32];
    length[..8].copy_from_slice(&len.to_le_bytes());
    items.push(length);
    for chunk in bytes.chunks(KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1) {
        let mut item = [0_u8; 32];
        item[..chunk.len()].copy_from_slice(chunk);
        items.push(item);
    }
    items
}

/// `P_bytes(domain, bytes)`: Poseidon over the packed bytes (§3).
#[must_use]
pub fn kagemusha_wallet_poseidon_bytes_v1(domain: u64, bytes: &[u8]) -> [u8; 32] {
    poseidon_items_v1(domain, &kagemusha_wallet_packed_bytes_v1(bytes))
}

// ---------------------------------------------------------------------------------------
// Sparse Merkle tree (depth 256)
// ---------------------------------------------------------------------------------------

/// Sparse-tree node `P(kgwsmtn1, [left, right])` over canonical children.
///
/// # Errors
///
/// Rejects a noncanonical child.
pub fn kagemusha_wallet_sparse_node_v1(
    left: &[u8; 32],
    right: &[u8; 32],
) -> WalletResult<[u8; 32]> {
    kagemusha_wallet_poseidon_v1(KAGEMUSHA_WALLET_SPARSE_NODE_DOMAIN_V1, &[*left, *right])
}

fn sparse_defaults_v1() -> &'static [[u8; 32]; KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 + 1] {
    static DEFAULTS: OnceLock<[[u8; 32]; KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 + 1]> =
        OnceLock::new();
    DEFAULTS.get_or_init(|| {
        let mut defaults = [[0_u8; 32]; KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 + 1];
        defaults[0] = poseidon_items_v1(KAGEMUSHA_WALLET_SPARSE_EMPTY_DOMAIN_V1, &[]);
        for height in 0..KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 {
            let child = defaults[height];
            defaults[height + 1] =
                poseidon_items_v1(KAGEMUSHA_WALLET_SPARSE_NODE_DOMAIN_V1, &[child, child]);
        }
        defaults
    })
}

/// Root of the empty subtree of `height` (0 is the empty leaf, 256 the empty tree).
///
/// # Errors
///
/// Rejects a height above [`KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1`].
pub fn kagemusha_wallet_sparse_default_v1(height: usize) -> WalletResult<[u8; 32]> {
    sparse_defaults_v1()
        .get(height)
        .copied()
        .ok_or_else(|| invalid_v1("sparse_tree.height"))
}

/// Root of the empty depth-256 tree: the empty root of every state map and the Λ base-case
/// credit-digest root (§§3, 3.2).
#[must_use]
pub fn kagemusha_wallet_empty_map_root_v1() -> [u8; 32] {
    sparse_defaults_v1()[KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1]
}

/// Map key of an integer pair `(high, low)`: `high · 2^128 + low`, a canonical element for every
/// `high < 2^125`. The load/redeem recovery map uses `(kind, ordinal)` (owner answer Q3) and the
/// quota-usage map `(window kind, window start)`.
#[must_use]
pub fn kagemusha_wallet_pair_key_v1(high: u8, low: u128) -> [u8; 32] {
    let mut key = [0_u8; 32];
    key[..16].copy_from_slice(&low.to_le_bytes());
    key[16] = high;
    key
}

fn bit_v1(key: &[u8; 32], height: usize) -> bool {
    (key[height / 8] >> (height % 8)) & 1 == 1
}

fn shift_right_v1(index: [u64; 4]) -> [u64; 4] {
    [
        (index[0] >> 1) | (index[1] << 63),
        (index[1] >> 1) | (index[2] << 63),
        (index[2] >> 1) | (index[3] << 63),
        index[3] >> 1,
    ]
}

fn sibling_index_v1(index: [u64; 4]) -> [u64; 4] {
    [index[0] ^ 1, index[1], index[2], index[3]]
}

/// Compressed opening of one key in a depth-256 sparse tree: the presence bitmap of the
/// non-default siblings and exactly those siblings, root-ward (§3, owner answer Q7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KagemushaWalletSparseOpeningV1 {
    /// Bit `h` (byte `h / 8`, least significant bit first) marks a non-default sibling at
    /// height `h`.
    pub path_bitmap: [u8; 32],
    /// The non-default siblings in increasing height order.
    pub siblings: Vec<[u8; 32]>,
}

impl KagemushaWalletSparseOpeningV1 {
    /// Recompute the root from `key` and the leaf value at `key` (the empty leaf for an absent
    /// key).
    ///
    /// # Errors
    ///
    /// Rejects a noncanonical key, leaf or sibling, a sibling count other than the bitmap's
    /// population count, and a present sibling equal to the default subtree of its height (a
    /// non-canonical opening).
    pub fn root(&self, key: &[u8; 32], leaf: &[u8; 32]) -> WalletResult<[u8; 32]> {
        if !kagemusha_wallet_is_canonical_field_v1(key) {
            return Err(invalid_v1("sparse_opening.key"));
        }
        if !kagemusha_wallet_is_canonical_field_v1(leaf) {
            return Err(invalid_v1("sparse_opening.leaf"));
        }
        let present: u32 = self.path_bitmap.iter().map(|byte| byte.count_ones()).sum();
        if usize::try_from(present).ok() != Some(self.siblings.len()) {
            return Err(invalid_v1("sparse_opening.siblings"));
        }
        let defaults = sparse_defaults_v1();
        let mut siblings = self.siblings.iter();
        let mut node = *leaf;
        for (height, default) in defaults
            .iter()
            .take(KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1)
            .enumerate()
        {
            let sibling = if bit_v1(&self.path_bitmap, height) {
                let sibling = siblings
                    .next()
                    .ok_or_else(|| invalid_v1("sparse_opening.siblings"))?;
                if !kagemusha_wallet_is_canonical_field_v1(sibling) || sibling == default {
                    return Err(invalid_v1("sparse_opening.sibling"));
                }
                *sibling
            } else {
                *default
            };
            let pair = if bit_v1(key, height) {
                [sibling, node]
            } else {
                [node, sibling]
            };
            node = poseidon_items_v1(KAGEMUSHA_WALLET_SPARSE_NODE_DOMAIN_V1, &pair);
        }
        Ok(node)
    }
}

/// Native depth-256 sparse Merkle tree of canonical keys and leaf values (§3).
///
/// This is a native helper of the map owner and of the credit-digest opening; its root and
/// openings are the in-circuit values. Leaf values are the `P` leaf hashes of the map's leaf
/// type; an absent key holds the empty leaf.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KagemushaWalletSparseTreeV1 {
    leaves: BTreeMap<[u8; 32], [u8; 32]>,
}

/// Occupied nodes of every height, leaves first (height 0) and the root last (height 256).
type SparseLevelsV1 = Vec<BTreeMap<[u64; 4], [u8; 32]>>;

impl KagemushaWalletSparseTreeV1 {
    /// An empty tree.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace the leaf value at `key` and return the replaced value.
    ///
    /// # Errors
    ///
    /// Rejects a noncanonical key or leaf value.
    pub fn insert(&mut self, key: [u8; 32], leaf: [u8; 32]) -> WalletResult<Option<[u8; 32]>> {
        if !kagemusha_wallet_is_canonical_field_v1(&key) {
            return Err(invalid_v1("sparse_tree.key"));
        }
        if !kagemusha_wallet_is_canonical_field_v1(&leaf) {
            return Err(invalid_v1("sparse_tree.leaf"));
        }
        Ok(self.leaves.insert(key, leaf))
    }

    /// Remove the leaf at `key` and return its value.
    pub fn remove(&mut self, key: &[u8; 32]) -> Option<[u8; 32]> {
        self.leaves.remove(key)
    }

    /// Leaf value at `key`, if present.
    #[must_use]
    pub fn get(&self, key: &[u8; 32]) -> Option<[u8; 32]> {
        self.leaves.get(key).copied()
    }

    /// Number of present leaves.
    #[must_use]
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Whether the tree has no leaf.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    fn levels(&self) -> SparseLevelsV1 {
        let defaults = sparse_defaults_v1();
        let mut levels: SparseLevelsV1 =
            Vec::with_capacity(KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 + 1);
        let mut level: BTreeMap<[u64; 4], [u8; 32]> = self
            .leaves
            .iter()
            .map(|(key, leaf)| (limbs_of_v1(key), *leaf))
            .collect();
        for default in defaults.iter().take(KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1) {
            let mut next = BTreeMap::new();
            for (index, node) in &level {
                let parent = shift_right_v1(*index);
                if next.contains_key(&parent) {
                    continue;
                }
                let sibling = level
                    .get(&sibling_index_v1(*index))
                    .copied()
                    .unwrap_or(*default);
                let pair = if index[0] & 1 == 1 {
                    [sibling, *node]
                } else {
                    [*node, sibling]
                };
                next.insert(
                    parent,
                    poseidon_items_v1(KAGEMUSHA_WALLET_SPARSE_NODE_DOMAIN_V1, &pair),
                );
            }
            levels.push(level);
            level = next;
        }
        levels.push(level);
        levels
    }

    /// Root of the tree; the empty root when no leaf is present.
    #[must_use]
    pub fn root(&self) -> [u8; 32] {
        if self.leaves.is_empty() {
            return kagemusha_wallet_empty_map_root_v1();
        }
        self.levels()
            .last()
            .and_then(|root| root.values().next().copied())
            .unwrap_or_else(kagemusha_wallet_empty_map_root_v1)
    }

    /// Compressed opening of `key`: a membership opening of its leaf when present, otherwise an
    /// absence opening whose leaf is the empty leaf.
    ///
    /// # Errors
    ///
    /// Rejects a noncanonical key.
    pub fn opening(&self, key: &[u8; 32]) -> WalletResult<KagemushaWalletSparseOpeningV1> {
        if !kagemusha_wallet_is_canonical_field_v1(key) {
            return Err(invalid_v1("sparse_tree.key"));
        }
        let levels = self.levels();
        let mut path_bitmap = [0_u8; 32];
        let mut siblings = Vec::new();
        let mut index = limbs_of_v1(key);
        for (height, level) in levels
            .iter()
            .take(KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1)
            .enumerate()
        {
            if let Some(sibling) = level.get(&sibling_index_v1(index)) {
                path_bitmap[height / 8] |= 1 << (height % 8);
                siblings.push(*sibling);
            }
            index = shift_right_v1(index);
        }
        Ok(KagemushaWalletSparseOpeningV1 {
            path_bitmap,
            siblings,
        })
    }

    /// Verify that `opening` proves `leaf` at `key` under `root`.
    ///
    /// # Errors
    ///
    /// Rejects what [`KagemushaWalletSparseOpeningV1::root`] rejects and another root.
    pub fn verify_membership(
        root: &[u8; 32],
        key: &[u8; 32],
        leaf: &[u8; 32],
        opening: &KagemushaWalletSparseOpeningV1,
    ) -> WalletResult<()> {
        if opening.root(key, leaf)? == *root {
            Ok(())
        } else {
            Err(invalid_v1("sparse_opening.root"))
        }
    }

    /// Verify that `opening` proves `key` absent (its leaf is the empty leaf) under `root`.
    ///
    /// # Errors
    ///
    /// Rejects what [`KagemushaWalletSparseOpeningV1::root`] rejects and another root.
    pub fn verify_absence(
        root: &[u8; 32],
        key: &[u8; 32],
        opening: &KagemushaWalletSparseOpeningV1,
    ) -> WalletResult<()> {
        let empty = sparse_defaults_v1()[0];
        Self::verify_membership(root, key, &empty, opening)
    }
}
