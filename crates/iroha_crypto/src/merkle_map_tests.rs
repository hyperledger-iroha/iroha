//! Canonical map commitments, adversarial paths and persistent-version controls.

use super::*;
use std::collections::BTreeMap;

fn hash(n: u64) -> Hash {
    Hash::new(n.to_le_bytes())
}

// Rebuild from sorted leaves, independent of the incremental tree and its
// insertion/removal helpers. Split each set by its first differing bit.
fn reference(entries: &BTreeMap<Hash, Hash>) -> Hash {
    fn subtree(entries: &[(&Hash, &Hash)]) -> Hash {
        if entries.len() == 1 {
            return Hash::new_from_chunks(&[LEAF, entries[0].0.as_ref(), entries[0].1.as_ref()]);
        }
        let first = entries[0].0.as_ref();
        let last = entries[entries.len() - 1].0.as_ref();
        let bit = (0..256_usize)
            .find(|&b| (first[b / 8] ^ last[b / 8]) & (128 >> (b % 8)) != 0)
            .unwrap();
        let boundary =
            entries.partition_point(|(key, _)| key.as_ref()[bit / 8] & (128 >> (bit % 8)) == 0);
        let mut shared = [0_u8; 32];
        for b in 0..bit {
            shared[b / 8] |= first[b / 8] & (128 >> (b % 8));
        }
        Hash::new_from_chunks(&[
            BRANCH,
            &(bit as u16).to_le_bytes(),
            &shared,
            subtree(&entries[..boundary]).as_ref(),
            subtree(&entries[boundary..]).as_ref(),
        ])
    }
    let inner = if entries.is_empty() {
        Hash::new(EMPTY)
    } else {
        subtree(&entries.iter().collect::<Vec<_>>())
    };
    Hash::new_from_chunks(&[ROOT, &(entries.len() as u64).to_le_bytes(), inner.as_ref()])
}

#[test]
fn all_insertion_orders_and_deletion_histories_have_the_same_root() {
    fn permutations(values: &mut [u64], at: usize, test: &mut impl FnMut(&[u64])) {
        if at == values.len() {
            test(values);
            return;
        }
        for next in at..values.len() {
            values.swap(at, next);
            permutations(values, at + 1, test);
            values.swap(at, next);
        }
    }
    let entries = (0..5)
        .map(|n| (hash(n), hash(n + 10)))
        .collect::<BTreeMap<_, _>>();
    let expected = reference(&entries);
    let mut count = 0;
    permutations(&mut [0, 1, 2, 3, 4], 0, &mut |order| {
        let mut map = MerkleMap::new();
        assert!(map.is_empty());
        for &n in order {
            assert!(map.replace(hash(n), None, Some(hash(n + 10))).unwrap());
        }
        assert_eq!(map.root(), expected);
        assert_eq!(map.len(), 5);
        let snapshot = map.clone();
        let mut remaining = entries.clone();
        for &n in order {
            map.replace(hash(n), Some(hash(n + 10)), None).unwrap();
            remaining.remove(&hash(n));
            assert_eq!(map.root(), reference(&remaining));
            assert_eq!(snapshot.root(), expected);
        }
        assert!(map.is_empty());
        assert_eq!(map.root(), MerkleMap::default().root());
        count += 1;
    });
    assert_eq!(count, 120);
}

#[test]
fn stale_preimages_noops_and_count_failures_do_not_change_versions() {
    let mut map = MerkleMap::new();
    map.replace(hash(1), None, Some(hash(2))).unwrap();
    let before = map.clone();
    assert_eq!(
        map.replace(hash(1), None, None),
        Err(MerkleMapError::PreimageMismatch {
            expected: None,
            actual: Some(hash(2))
        })
    );
    assert!(map.replace(hash(9), Some(hash(2)), Some(hash(3))).is_err());
    assert!(!map.replace(hash(1), Some(hash(2)), Some(hash(2))).unwrap());
    assert!(!map.replace(hash(9), None, None).unwrap());
    assert_eq!(map.root(), before.root());
    assert!(Arc::ptr_eq(
        map.node.as_ref().unwrap(),
        before.node.as_ref().unwrap()
    ));
    map.len = u64::MAX;
    let overflow_root = map.root();
    assert_eq!(
        map.replace(hash(9), None, Some(hash(3))),
        Err(MerkleMapError::Capacity)
    );
    assert_eq!(map.root(), overflow_root);
    assert_eq!(map.get(&hash(9)), None);
}

#[test]
fn every_split_bit_and_byte_boundary_matches_the_rebuilt_reference() {
    let zero = Hash::prehashed([0; 32]);
    let mut map = MerkleMap::new();
    let mut expected = BTreeMap::new();
    map.replace(zero, None, Some(hash(999))).unwrap();
    expected.insert(zero, hash(999));
    // The last bit is Hash's fixed marker, so all 255 variable bit positions
    // occur, including the deepest possible branch and every byte boundary.
    for bit in (0..255).rev() {
        let mut bytes = [0; 32];
        bytes[bit / 8] = 128 >> (bit % 8);
        let key = Hash::prehashed(bytes);
        map.replace(key, None, Some(hash(bit as u64))).unwrap();
        expected.insert(key, hash(bit as u64));
        assert_eq!(map.root(), reference(&expected));
    }
    for (key, value) in &expected {
        assert_eq!(map.get(key), Some(*value));
    }
    fn height(node: &Node) -> usize {
        match &node.kind {
            NodeKind::Leaf(_) => 0,
            NodeKind::Branch { left, right, .. } => 1 + height(left).max(height(right)),
        }
    }
    assert_eq!(height(map.node.as_ref().unwrap()), 255);
    map.replace(zero, Some(hash(999)), None).unwrap();
    expected.remove(&zero);
    assert_eq!(map.root(), reference(&expected));
    assert_eq!(map.get(&zero), None);
}

#[test]
fn mixed_mutations_match_a_sorted_map_without_changing_snapshots() {
    let mut map = MerkleMap::new();
    let mut expected = BTreeMap::new();
    let mut rng = 0x1234_5678_9abc_def0_u64;
    for step in 0..700_u64 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let key = hash(rng % 63);
        let before = expected.get(&key).copied();
        let after = (rng % 4 != 0).then(|| hash(step));
        let snapshot = map.clone();
        let old_root = snapshot.root();
        map.replace(key, before, after).unwrap();
        match after {
            Some(value) => {
                expected.insert(key, value);
            }
            None => {
                expected.remove(&key);
            }
        }
        assert_eq!(snapshot.root(), old_root);
        assert_eq!(snapshot.get(&key), before);
        assert_eq!(map.root(), reference(&expected), "step {step}");
        assert_eq!(map.len(), expected.len() as u64);
        assert_eq!(map.get(&key), after);
    }
}

#[test]
fn unrelated_subtrees_are_shared_and_key_value_bindings_are_distinct() {
    let left_key = Hash::prehashed([0; 32]);
    let right_key = Hash::prehashed([255; 32]);
    let mut map = MerkleMap::new();
    map.replace(left_key, None, Some(hash(1))).unwrap();
    map.replace(right_key, None, Some(hash(2))).unwrap();
    let before = map.clone();
    map.replace(left_key, Some(hash(1)), Some(hash(3))).unwrap();
    match (
        &before.node.as_ref().unwrap().kind,
        &map.node.as_ref().unwrap().kind,
    ) {
        (
            NodeKind::Branch {
                left: old_left,
                right: old_right,
                ..
            },
            NodeKind::Branch { left, right, .. },
        ) => {
            assert!(Arc::ptr_eq(old_right, right));
            assert!(!Arc::ptr_eq(old_left, left));
        }
        _ => panic!("two keys require a branch"),
    }
    assert_ne!(before.root(), map.root());
    let mut swapped = MerkleMap::new();
    swapped.replace(left_key, None, Some(hash(2))).unwrap();
    swapped.replace(right_key, None, Some(hash(1))).unwrap();
    assert_ne!(before.root(), swapped.root());
    let mut singleton = MerkleMap::new();
    singleton.replace(left_key, None, Some(hash(1))).unwrap();
    assert_ne!(singleton.root(), before.root());
    assert_ne!(singleton.root(), MerkleMap::new().root());
}

#[test]
fn fixed_blake2b_vectors_bind_empty_leaf_and_root_branch() {
    // Independently calculated with Python hashlib.blake2b(digest_size=32),
    // applying Iroha's low-bit marker after every hash.
    let mut map = MerkleMap::new();
    assert_eq!(
        map.root().to_string(),
        "4ed9ccac2f64fe8467c3b9a56d5ba2b842622208380bfaaa4fe6fc22d53acbf5"
    );
    map.replace(Hash::prehashed([0; 32]), None, Some(Hash::new(b"a")))
        .unwrap();
    assert_eq!(
        map.root().to_string(),
        "a4bdd95126dc257edffbb9cd2a91afa3be1595c8d14551ee2fa8523ac0bc5c0f"
    );
    let mut right = [0; 32];
    right[0] = 128;
    map.replace(Hash::prehashed(right), None, Some(Hash::new(b"b")))
        .unwrap();
    assert_eq!(
        map.root().to_string(),
        "130c60ac5becbc55b3a871805f9cf4f621ad984440ef83fe025f68aadc4b4ee5"
    );
}

#[test]
fn exact_membership_proof_binds_key_value_count_and_canonical_path() {
    let mut map = MerkleMap::new();
    for n in 0..9 {
        map.replace(hash(n), None, Some(hash(n + 100))).unwrap();
    }
    assert!(map.proof(&hash(99)).is_none());
    for n in 0..9 {
        let proof = map.proof(&hash(n)).expect("present key has proof");
        assert!(proof.verify(map.root()));
        let encoded = norito::codec::Encode::encode(&proof);
        let decoded = norito::codec::Decode::decode_all(&mut encoded.as_slice())
            .expect("membership proof decodes");
        assert_eq!(proof, decoded);
        let mut changed = proof.clone();
        changed.value = hash(999);
        assert!(!changed.verify(map.root()));
        let mut changed = proof.clone();
        changed.key = hash(999);
        assert!(!changed.verify(map.root()));
        let mut changed = proof.clone();
        changed.len += 1;
        assert!(!changed.verify(map.root()));
        let mut changed = proof;
        changed.steps[0].bit = 256;
        assert!(!changed.verify(map.root()));
    }
    let snapshot = map.clone();
    map.replace(hash(0), Some(hash(100)), Some(hash(200)))
        .unwrap();
    assert!(snapshot.proof(&hash(0)).unwrap().verify(snapshot.root()));
    assert!(!snapshot.proof(&hash(0)).unwrap().verify(map.root()));
}
