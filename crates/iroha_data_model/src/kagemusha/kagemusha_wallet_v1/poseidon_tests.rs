//! σ-field Poseidon, packed-byte and sparse-tree tests.

use super::*;
use crate::kagemusha::kagemusha_wallet_v1::{
    KagemushaWalletValidationErrorV1,
    digest::{KAGEMUSHA_WALLET_FIELD_MODULUS_V1, kagemusha_wallet_field_from_u128_v1},
};

fn int(value: u128) -> [u8; 32] {
    kagemusha_wallet_field_from_u128_v1(value)
}

fn hex32(text: &str) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value.copy_from_slice(&hex::decode(text).expect("hex"));
    value
}

#[track_caller]
fn assert_invalid<T: core::fmt::Debug>(result: WalletResult<T>, expected: &str) {
    match result {
        Err(KagemushaWalletValidationErrorV1::InvalidField { field }) if field == expected => {}
        other => panic!("expected invalid `{expected}`, got {other:?}"),
    }
}

/// A canonical stand-in key with `seed` in every byte below the most significant.
fn key(seed: u8) -> [u8; 32] {
    let mut value = [seed; 32];
    value[31] = seed & 0x3f;
    value
}

/// Leaf value of a stand-in leaf.
fn leaf(seed: u8) -> [u8; 32] {
    kagemusha_wallet_poseidon_v1(
        KAGEMUSHA_WALLET_CREDIT_DIGEST_LEAF_DOMAIN_V1,
        &[int(seed.into())],
    )
    .expect("leaf")
}

fn flip(value: &[u8; 32], height: usize) -> [u8; 32] {
    let mut flipped = *value;
    flipped[height / 8] ^= 1 << (height % 8);
    flipped
}

#[test]
fn kagemusha_wallet_v1_poseidon_domains_are_distinct_ascii_words() {
    let mut seen = std::collections::BTreeSet::new();
    for (name, domain) in KAGEMUSHA_WALLET_POSEIDON_DOMAINS_V1 {
        let ascii = domain.to_le_bytes();
        assert!(ascii.iter().all(u8::is_ascii_graphic), "{name}");
        assert_eq!(&ascii[..3], b"kgw", "{name}");
        assert!(seen.insert(domain), "{name}");
    }
    for (domain, ascii) in [
        (KAGEMUSHA_WALLET_CORE_DOMAIN_V1, b"kgwcore1"),
        (KAGEMUSHA_WALLET_REST_DOMAIN_V1, b"kgwrest1"),
        (KAGEMUSHA_WALLET_STATEMENT_DOMAIN_V1, b"kgwstmt1"),
        (KAGEMUSHA_WALLET_CREDIT_DOMAIN_V1, b"kgwcrdt1"),
        (KAGEMUSHA_WALLET_SEND_CHAIN_DOMAIN_V1, b"kgwschn1"),
        (KAGEMUSHA_WALLET_RECV_CHAIN_DOMAIN_V1, b"kgwrchn1"),
        (KAGEMUSHA_WALLET_CONSUMED_CREDIT_LEAF_DOMAIN_V1, b"kgwccrd1"),
        (
            KAGEMUSHA_WALLET_PENDING_OUTGOING_LEAF_DOMAIN_V1,
            b"kgwpout1",
        ),
        (KAGEMUSHA_WALLET_LOAD_RECOVERY_LEAF_DOMAIN_V1, b"kgwload1"),
        (KAGEMUSHA_WALLET_REDEEM_RECOVERY_LEAF_DOMAIN_V1, b"kgwrdm_1"),
        (KAGEMUSHA_WALLET_FEE_CLAIM_LEAF_DOMAIN_V1, b"kgwfee_1"),
        (KAGEMUSHA_WALLET_QUOTA_USAGE_LEAF_DOMAIN_V1, b"kgwquse1"),
        (KAGEMUSHA_WALLET_CREDIT_DIGEST_LEAF_DOMAIN_V1, b"kgwcdig1"),
        (KAGEMUSHA_WALLET_SPARSE_EMPTY_DOMAIN_V1, b"kgwsmte1"),
        (KAGEMUSHA_WALLET_SPARSE_NODE_DOMAIN_V1, b"kgwsmtn1"),
        (KAGEMUSHA_WALLET_BLACKLIST_LEAF_DOMAIN_V1, b"kgwblkl1"),
        (KAGEMUSHA_WALLET_BLACKLIST_NODE_DOMAIN_V1, b"kgwblkn1"),
        (KAGEMUSHA_WALLET_QUOTA_WINDOW_DOMAIN_V1, b"kgwqwin1"),
        (KAGEMUSHA_WALLET_QUOTA_NODE_DOMAIN_V1, b"kgwqwnd1"),
        (KAGEMUSHA_WALLET_PROOF_DOMAIN_V1, b"kgwprf_1"),
        (KAGEMUSHA_WALLET_STEP_PROOF_DOMAIN_V1, b"kgwstep1"),
        (KAGEMUSHA_WALLET_PAYMENT_DOMAIN_V1, b"kgwpay_1"),
    ] {
        assert_eq!(domain.to_le_bytes(), *ascii);
        assert!(
            KAGEMUSHA_WALLET_POSEIDON_DOMAINS_V1
                .iter()
                .any(|(_, listed)| *listed == domain)
        );
    }
    assert_eq!(seen.len(), KAGEMUSHA_WALLET_POSEIDON_DOMAINS_V1.len());
}

/// `P` reproduces the KAGEMUSHA domain-hash vectors of the vendored sponge
/// (`fixtures/native_prover/kats_v1.json`, `kagemusha_v1_poseidon.fp`).
#[test]
fn kagemusha_wallet_v1_poseidon_reproduces_the_native_prover_kats() {
    use norito::json::Value;
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/native_prover/kats_v1.json");
    let text = std::fs::read_to_string(path).expect("kats_v1.json");
    let fixture = norito::json::parse_value(&text).expect("json");
    let fp = fixture
        .get("kagemusha_v1_poseidon")
        .and_then(|section| section.get("fp"))
        .expect("fp section");
    let Some(Value::Array(vectors)) = fp.get("vectors") else {
        panic!("vectors");
    };
    assert!(vectors.len() >= 5);
    for vector in vectors {
        let ascii = vector
            .get("domain")
            .and_then(Value::as_str)
            .expect("domain");
        let mut word = [0_u8; 8];
        word.copy_from_slice(ascii.as_bytes());
        let Some(Value::Array(inputs)) = vector.get("inputs") else {
            panic!("inputs");
        };
        let inputs: Vec<[u8; 32]> = inputs
            .iter()
            .map(|input| hex32(input.as_str().expect("input")))
            .collect();
        let output = hex32(
            vector
                .get("output")
                .and_then(Value::as_str)
                .expect("output"),
        );
        let domain = u64::from_le_bytes(word);
        assert_eq!(
            kagemusha_wallet_poseidon_v1(domain, &inputs).expect("canonical inputs"),
            output,
            "{ascii}"
        );
        assert_eq!(poseidon_items_v1(domain, &inputs), output, "{ascii}");
    }
    // The empty depth-256 replay root of the same construction (another domain pair).
    let empty = u64::from_le_bytes(*b"kgmemp_1");
    let node = u64::from_le_bytes(*b"kgmnode1");
    let mut root = kagemusha_wallet_poseidon_v1(empty, &[]).expect("empty");
    for _ in 0..KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 {
        root = kagemusha_wallet_poseidon_v1(node, &[root, root]).expect("node");
    }
    let expected = fp
        .get("empty_replay_root")
        .and_then(Value::as_str)
        .expect("empty root");
    assert_eq!(root, hex32(expected));
}

#[test]
fn kagemusha_wallet_v1_poseidon_rejects_noncanonical_items() {
    assert_invalid(
        kagemusha_wallet_poseidon_v1(
            KAGEMUSHA_WALLET_CORE_DOMAIN_V1,
            &[KAGEMUSHA_WALLET_FIELD_MODULUS_V1],
        ),
        "poseidon.item",
    );
    assert_invalid(
        kagemusha_wallet_poseidon_v1(KAGEMUSHA_WALLET_CORE_DOMAIN_V1, &[int(1), [0xff; 32]]),
        "poseidon.item",
    );
    let mut below = KAGEMUSHA_WALLET_FIELD_MODULUS_V1;
    below[0] = 0;
    kagemusha_wallet_poseidon_v1(KAGEMUSHA_WALLET_CORE_DOMAIN_V1, &[below]).expect("p - 1");
    // The domain and the arity separate equal element lists.
    let one = kagemusha_wallet_poseidon_v1(KAGEMUSHA_WALLET_CORE_DOMAIN_V1, &[int(1)]).expect("a");
    assert_ne!(
        one,
        kagemusha_wallet_poseidon_v1(KAGEMUSHA_WALLET_REST_DOMAIN_V1, &[int(1)]).expect("b")
    );
    assert_ne!(
        one,
        kagemusha_wallet_poseidon_v1(KAGEMUSHA_WALLET_CORE_DOMAIN_V1, &[int(1), int(0)])
            .expect("c")
    );
}

/// The `P_bytes` packing rule: the length element, then 31-byte little-endian chunks, the
/// last zero-filled; empty input and the 31-byte chunk boundaries.
#[test]
fn kagemusha_wallet_v1_packed_bytes_rule() {
    assert_eq!(KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1, 31);
    let bytes: Vec<u8> = (1..=63_u8).collect();
    for (len, elements) in [(0, 1), (1, 2), (30, 2), (31, 2), (32, 3), (62, 3), (63, 4)] {
        let items = kagemusha_wallet_packed_bytes_v1(&bytes[..len]);
        assert_eq!(items.len(), elements, "len {len}");
        assert_eq!(
            items[0],
            int(u128::try_from(len).expect("len")),
            "len {len}"
        );
        for (index, item) in items.iter().enumerate().skip(1) {
            let start = (index - 1) * 31;
            let end = (start + 31).min(len);
            let mut expected = [0_u8; 32];
            expected[..end - start].copy_from_slice(&bytes[start..end]);
            assert_eq!(*item, expected, "len {len} chunk {index}");
            assert_eq!(item[31], 0);
        }
        assert_eq!(
            kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_PAYMENT_DOMAIN_V1, &bytes[..len]),
            kagemusha_wallet_poseidon_v1(KAGEMUSHA_WALLET_PAYMENT_DOMAIN_V1, &items)
                .expect("packed items are canonical"),
            "len {len}"
        );
    }
    // A full chunk of 0xff is still canonical (below 2^248).
    let full = kagemusha_wallet_packed_bytes_v1(&[0xff; 31]);
    assert_eq!(full[1][..31], [0xff; 31]);
    // The length element separates inputs that pack to equal chunks.
    assert_ne!(
        kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_PAYMENT_DOMAIN_V1, &[0]),
        kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_PAYMENT_DOMAIN_V1, &[0, 0])
    );
    assert_ne!(
        kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_PROOF_DOMAIN_V1, &[]),
        kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_STEP_PROOF_DOMAIN_V1, &[])
    );
}

#[test]
fn kagemusha_wallet_v1_sparse_tree_defaults_and_empty_root() {
    let empty_leaf = kagemusha_wallet_sparse_default_v1(0).expect("leaf");
    assert_eq!(
        empty_leaf,
        kagemusha_wallet_poseidon_v1(KAGEMUSHA_WALLET_SPARSE_EMPTY_DOMAIN_V1, &[]).expect("leaf")
    );
    let mut node = empty_leaf;
    for height in 1..=KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 {
        node = kagemusha_wallet_sparse_node_v1(&node, &node).expect("node");
        assert_eq!(kagemusha_wallet_sparse_default_v1(height).ok(), Some(node));
    }
    assert_eq!(kagemusha_wallet_empty_map_root_v1(), node);
    assert_eq!(KagemushaWalletSparseTreeV1::new().root(), node);
    assert_invalid(
        kagemusha_wallet_sparse_default_v1(KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 + 1),
        "sparse_tree.height",
    );
    assert_invalid(
        kagemusha_wallet_sparse_node_v1(&[0xff; 32], &node),
        "poseidon.item",
    );
}

#[test]
fn kagemusha_wallet_v1_pair_keys_are_canonical_and_injective() {
    let key = kagemusha_wallet_pair_key_v1(2, 0x0102);
    let mut expected = [0_u8; 32];
    expected[..16].copy_from_slice(&0x0102_u128.to_le_bytes());
    expected[16] = 2;
    assert_eq!(key, expected);
    assert!(
        super::super::digest::kagemusha_wallet_is_canonical_field_v1(
            &kagemusha_wallet_pair_key_v1(u8::MAX, u128::MAX)
        )
    );
    assert_ne!(
        kagemusha_wallet_pair_key_v1(1, 5),
        kagemusha_wallet_pair_key_v1(2, 5)
    );
    assert_ne!(
        kagemusha_wallet_pair_key_v1(1, 5),
        kagemusha_wallet_pair_key_v1(1, 6)
    );
}

#[test]
fn kagemusha_wallet_v1_sparse_tree_root_by_hand() {
    // Two keys that differ only in bit 0 are siblings at height 0.
    let a = key(0x10);
    let b = flip(&a, 0);
    let mut tree = KagemushaWalletSparseTreeV1::new();
    assert_eq!(tree.insert(a, leaf(1)).ok(), Some(None));
    assert_eq!(tree.insert(b, leaf(2)).ok(), Some(None));
    assert_eq!(tree.len(), 2);
    assert!(!tree.is_empty());
    let (left, right) = if a[0] & 1 == 0 {
        (leaf(1), leaf(2))
    } else {
        (leaf(2), leaf(1))
    };
    let mut node = kagemusha_wallet_sparse_node_v1(&left, &right).expect("node");
    for height in 1..KAGEMUSHA_WALLET_SPARSE_TREE_DEPTH_V1 {
        let default = kagemusha_wallet_sparse_default_v1(height).expect("default");
        let bit = (a[height / 8] >> (height % 8)) & 1;
        node = if bit == 1 {
            kagemusha_wallet_sparse_node_v1(&default, &node)
        } else {
            kagemusha_wallet_sparse_node_v1(&node, &default)
        }
        .expect("node");
    }
    assert_eq!(tree.root(), node);
    // Replacing a leaf returns the old value and changes the root.
    assert_eq!(tree.insert(a, leaf(3)).ok(), Some(Some(leaf(1))));
    assert_ne!(tree.root(), node);
    assert_eq!(tree.get(&a), Some(leaf(3)));
    assert_eq!(tree.remove(&a), Some(leaf(3)));
    assert_eq!(tree.get(&a), None);
    assert_eq!(tree.remove(&b), Some(leaf(2)));
    assert_eq!(tree.root(), kagemusha_wallet_empty_map_root_v1());
    // Keys and leaves are canonical field values.
    assert_invalid(tree.insert([0xff; 32], leaf(1)), "sparse_tree.key");
    assert_invalid(tree.insert(a, [0xff; 32]), "sparse_tree.leaf");
}

#[test]
fn kagemusha_wallet_v1_sparse_tree_membership_and_absence_openings() {
    let base = key(0x5a);
    let mut tree = KagemushaWalletSparseTreeV1::new();
    tree.insert(base, leaf(1)).expect("base");
    // Neighbours at heights 0, 7 and 200 make exactly those siblings non-default.
    for (seed, height) in [(2, 0_usize), (3, 7), (4, 200)] {
        tree.insert(flip(&base, height), leaf(seed))
            .expect("neighbour");
    }
    let root = tree.root();
    let opening = tree.opening(&base).expect("opening");
    assert_eq!(opening.siblings.len(), 3);
    let mut bitmap = [0_u8; 32];
    for height in [0_usize, 7, 200] {
        bitmap[height / 8] |= 1 << (height % 8);
    }
    assert_eq!(opening.path_bitmap, bitmap);
    assert_eq!(opening.siblings[0], leaf(2));
    KagemushaWalletSparseTreeV1::verify_membership(&root, &base, &leaf(1), &opening)
        .expect("membership");
    assert_eq!(opening.root(&base, &leaf(1)).ok(), Some(root));
    for (height, key) in [
        (0_usize, flip(&base, 0)),
        (7, flip(&base, 7)),
        (200, flip(&base, 200)),
    ] {
        let neighbour = tree.opening(&key).expect("neighbour opening");
        assert!(neighbour.path_bitmap[height / 8] & (1 << (height % 8)) != 0);
        KagemushaWalletSparseTreeV1::verify_membership(
            &root,
            &key,
            &tree.get(&key).expect("leaf"),
            &neighbour,
        )
        .expect("neighbour membership");
    }

    // An absent key opens to the empty leaf; it is not a membership of any value.
    let absent = flip(&base, 3);
    let absence = tree.opening(&absent).expect("absence");
    KagemushaWalletSparseTreeV1::verify_absence(&root, &absent, &absence).expect("absence");
    assert_invalid(
        KagemushaWalletSparseTreeV1::verify_membership(&root, &absent, &leaf(1), &absence),
        "sparse_opening.root",
    );
    // A present key has no absence proof under its own opening.
    assert_invalid(
        KagemushaWalletSparseTreeV1::verify_absence(&root, &base, &opening),
        "sparse_opening.root",
    );
    // The same opening against another leaf or key fails.
    assert_invalid(
        KagemushaWalletSparseTreeV1::verify_membership(&root, &base, &leaf(9), &opening),
        "sparse_opening.root",
    );
    assert_invalid(
        KagemushaWalletSparseTreeV1::verify_membership(
            &root,
            &flip(&base, 100),
            &leaf(1),
            &opening,
        ),
        "sparse_opening.root",
    );

    // Tampered openings.
    let mut sibling = opening.clone();
    sibling.siblings[1] = leaf(7);
    assert_invalid(
        KagemushaWalletSparseTreeV1::verify_membership(&root, &base, &leaf(1), &sibling),
        "sparse_opening.root",
    );
    let mut missing = opening.clone();
    missing.siblings.pop();
    assert_invalid(missing.root(&base, &leaf(1)), "sparse_opening.siblings");
    let mut extra_bit = opening.clone();
    extra_bit.path_bitmap[1] |= 1;
    assert_invalid(extra_bit.root(&base, &leaf(1)), "sparse_opening.siblings");
    // Move the height-0 sibling to height 1 (bits 0 and 7 of byte 0 become bits 1 and 7).
    let mut moved = opening.clone();
    moved.path_bitmap[0] = 0b1000_0010;
    assert_invalid(
        KagemushaWalletSparseTreeV1::verify_membership(&root, &base, &leaf(1), &moved),
        "sparse_opening.root",
    );
    // A present sibling equal to its default subtree is a non-canonical second encoding.
    let mut padded = opening.clone();
    padded.path_bitmap[0] |= 0b10;
    padded
        .siblings
        .insert(1, kagemusha_wallet_sparse_default_v1(1).expect("default"));
    assert_invalid(padded.root(&base, &leaf(1)), "sparse_opening.sibling");
    let mut noncanonical = opening.clone();
    noncanonical.siblings[0] = [0xff; 32];
    assert_invalid(noncanonical.root(&base, &leaf(1)), "sparse_opening.sibling");
    assert_invalid(opening.root(&[0xff; 32], &leaf(1)), "sparse_opening.key");
    assert_invalid(opening.root(&base, &[0xff; 32]), "sparse_opening.leaf");
    assert_invalid(tree.opening(&[0xff; 32]), "sparse_tree.key");

    // The empty tree: every key is absent with no siblings.
    let empty = KagemushaWalletSparseTreeV1::new();
    let opening = empty.opening(&base).expect("empty opening");
    assert!(opening.siblings.is_empty());
    assert_eq!(opening.path_bitmap, [0; 32]);
    KagemushaWalletSparseTreeV1::verify_absence(
        &kagemusha_wallet_empty_map_root_v1(),
        &base,
        &opening,
    )
    .expect("empty absence");
}

#[test]
fn kagemusha_wallet_v1_sparse_tree_root_is_independent_of_insertion_order() {
    let keys: Vec<[u8; 32]> = (1..=6_u8).map(key).collect();
    let mut forward = KagemushaWalletSparseTreeV1::new();
    let mut backward = KagemushaWalletSparseTreeV1::new();
    for (index, key) in keys.iter().enumerate() {
        forward
            .insert(*key, leaf(u8::try_from(index).expect("seed")))
            .expect("insert");
    }
    for (index, key) in keys.iter().enumerate().rev() {
        backward
            .insert(*key, leaf(u8::try_from(index).expect("seed")))
            .expect("insert");
    }
    assert_eq!(forward.root(), backward.root());
    for (index, key) in keys.iter().enumerate() {
        let opening = forward.opening(key).expect("opening");
        KagemushaWalletSparseTreeV1::verify_membership(
            &forward.root(),
            key,
            &leaf(u8::try_from(index).expect("seed")),
            &opening,
        )
        .expect("membership");
    }
}
