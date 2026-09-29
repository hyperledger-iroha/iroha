//! Complete-range proof behavior over immutable map versions.

use super::*;

fn hash(first: u8) -> Hash {
    let mut bytes = [0_u8; Hash::LENGTH];
    bytes[0] = first;
    Hash::prehashed(bytes)
}

fn sample() -> (MerkleMap, Vec<(Hash, Hash)>) {
    let mut map = MerkleMap::new(&mv::allocation::AllocationBudget::new(64 * 1024 * 1024));
    let rows: Vec<_> = [3, 9, 10, 11, 18, 31, 32, 45, 55, 89]
        .into_iter()
        .map(|first| (hash(first), Hash::new([first])))
        .collect();
    for (key, value) in &rows {
        map.replace(*key, None, Some(*value))
            .expect("insert unique sample key");
    }
    (map, rows)
}

#[test]
fn range_proof_authenticates_every_row_and_empty_gaps() {
    let (map, rows) = sample();
    for start in 0..60 {
        for end in (start + 1)..61 {
            let start = hash(start);
            let end = hash(end);
            let expected: Vec<_> = rows
                .iter()
                .copied()
                .filter(|(key, _)| start <= *key && *key < end)
                .collect();
            let proof = map
                .prove_range(&start, &end, expected.len())
                .expect("bounded complete range");
            assert_eq!(proof.entry_count(), rows.len() as u64);
            assert_eq!(
                proof.verify(&map.root(), &start, &end, expected.len()),
                Ok(expected)
            );
        }
    }
    let empty = MerkleMap::new(&mv::allocation::AllocationBudget::new(64 * 1024 * 1024));
    let proof = empty
        .prove_range(&hash(0), &hash(255), 0)
        .expect("empty map range");
    assert_eq!(
        proof.verify(&empty.root(), &hash(0), &hash(255), 0),
        Ok(vec![])
    );
}

#[test]
fn range_proof_rejects_omitted_rows_bad_bounds_and_foreign_roots() {
    let (map, _) = sample();
    let start = hash(9);
    let end = hash(46);
    assert_eq!(
        map.prove_range(&start, &end, 1),
        Err(MerkleMapRangeError::Capacity)
    );
    assert_eq!(
        map.prove_range(&start, &start, 8),
        Err(MerkleMapRangeError::InvalidBounds)
    );
    assert_eq!(
        map.prove_range(&start, &end, MAX_RANGE_PROOF_ROWS + 1),
        Err(MerkleMapRangeError::InvalidLimit)
    );
    let proof = map.prove_range(&start, &end, 8).expect("bounded range");
    assert_eq!(
        proof.verify(&Hash::new(b"foreign range root"), &start, &end, 8),
        Err(MerkleMapRangeError::RootMismatch)
    );
    assert_eq!(
        proof.verify(&map.root(), &start, &end, 1),
        Err(MerkleMapRangeError::Capacity)
    );

    let mut omitted = proof.clone();
    for node in &mut omitted.nodes {
        if let Some(MerkleMapNode::Branch {
            bit,
            prefix,
            left,
            right,
        }) = node
        {
            let child = if partition_intersects(prefix, *bit, false, &start, &end) {
                left
            } else {
                right
            };
            child.location = PRUNED;
            break;
        }
    }
    assert_eq!(
        omitted.verify(&map.root(), &start, &end, 8),
        Err(MerkleMapRangeError::InvalidProof)
    );

    let mut extra = proof.clone();
    extra.nodes.push(proof.nodes[0]);
    assert_eq!(
        extra.verify(&map.root(), &start, &end, 8),
        Err(MerkleMapRangeError::InvalidProof)
    );
    let mut altered = proof.clone();
    let leaf = altered
        .nodes
        .iter_mut()
        .find_map(|node| match node {
            Some(MerkleMapNode::Leaf { value, .. }) => Some(value),
            _ => None,
        })
        .expect("expanded leaf");
    leaf.hash = Hash::new(b"altered range value");
    assert_eq!(
        altered.verify(&map.root(), &start, &end, 8),
        Err(MerkleMapRangeError::InvalidProof)
    );
}

#[test]
fn range_proof_keeps_older_root_after_map_mutation() {
    let (mut map, _) = sample();
    let start = hash(0);
    let end = hash(60);
    let original_root = map.root();
    let original = map.prove_range(&start, &end, 10).expect("old range");
    map.replace(hash(31), map.get(&hash(31)), None)
        .expect("remove existing key");
    map.replace(hash(25), None, Some(Hash::new(b"new value")))
        .expect("insert new key");
    assert_ne!(map.root(), original_root);
    assert_eq!(
        original
            .verify(&original_root, &start, &end, 10)
            .expect("old root remains valid")
            .len(),
        9
    );
    assert_eq!(
        original.verify(&map.root(), &start, &end, 10),
        Err(MerkleMapRangeError::RootMismatch)
    );
}

#[test]
fn range_proof_is_independent_of_insertion_order() {
    let (forward, rows) = sample();
    let mut reverse = MerkleMap::new(&mv::allocation::AllocationBudget::new(64 * 1024 * 1024));
    for (key, value) in rows.iter().rev() {
        reverse
            .replace(*key, None, Some(*value))
            .expect("insert reverse row");
    }
    assert_eq!(forward.root(), reverse.root());
    let start = hash(10);
    let end = hash(60);
    assert_eq!(
        forward.prove_range(&start, &end, 8),
        reverse.prove_range(&start, &end, 8)
    );
}

#[test]
fn range_proof_accepts_exact_row_ceiling_and_rejects_one_more() {
    let mut map = MerkleMap::new(&mv::allocation::AllocationBudget::new(64 * 1024 * 1024));
    let key = |index: u16| {
        let mut bytes = [0_u8; Hash::LENGTH];
        bytes[..2].copy_from_slice(&index.to_be_bytes());
        Hash::prehashed(bytes)
    };
    let start = key(0);
    let end = key(u16::MAX);
    for index in 0..MAX_RANGE_PROOF_ROWS {
        let index = u16::try_from(index).expect("fixed row ceiling fits u16");
        map.replace(key(index), None, Some(Hash::new(index.to_be_bytes())))
            .expect("insert a distinct row");
    }
    let proof = map
        .prove_range(&start, &end, MAX_RANGE_PROOF_ROWS)
        .expect("the exact row ceiling is admitted");
    assert_eq!(
        proof
            .verify(&map.root(), &start, &end, MAX_RANGE_PROOF_ROWS)
            .expect("the complete bounded proof verifies")
            .len(),
        MAX_RANGE_PROOF_ROWS
    );
    map.replace(
        key(u16::try_from(MAX_RANGE_PROOF_ROWS).expect("fixed row ceiling fits u16")),
        None,
        Some(Hash::new(b"one more row")),
    )
    .expect("insert the first excess row");
    assert_eq!(
        map.prove_range(&start, &end, MAX_RANGE_PROOF_ROWS),
        Err(MerkleMapRangeError::Capacity)
    );
}
