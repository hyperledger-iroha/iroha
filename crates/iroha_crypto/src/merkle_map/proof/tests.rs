//! Fixed-capacity path controls against an independent map root.

use super::*;

fn hash(n: u64) -> Hash {
    Hash::new(n.to_le_bytes())
}

#[test]
fn bounded_paths_authenticate_present_absent_and_retained_versions() {
    let mut map = MerkleMap::new(&iroha_allocation::AllocationBudget::new(64 * 1024 * 1024));
    let empty = map.prove_lookup(&hash(1));
    assert_eq!(empty.claimed_root(), map.root());
    assert_eq!(empty.verify(&map.root(), &hash(1)), Ok(None));
    assert_eq!(empty.used, 0);

    let mut values = Vec::new();
    for n in 0..48 {
        let key = hash(n);
        let value = hash(n + 100);
        map.replace(key, None, Some(value)).unwrap();
        values.push((key, value));
    }
    let original_root = map.root();
    let retained = map.prove_lookup(&values[21].0);
    assert_eq!(
        retained.verify(&original_root, &values[21].0),
        Ok(Some(values[21].1))
    );
    for (key, value) in values {
        let proof = map.prove_lookup(&key);
        assert_eq!(proof.claimed_root(), original_root);
        assert!(usize::from(proof.used) <= MAX_LOOKUP_PATH_NODES);
        assert_eq!(proof.verify(&original_root, &key), Ok(Some(value)));
    }
    for n in 48..96 {
        let key = hash(n);
        assert_eq!(
            map.prove_lookup(&key).verify(&original_root, &key),
            Ok(None)
        );
    }
    map.replace(hash(21), Some(hash(121)), Some(hash(999)))
        .unwrap();
    assert_eq!(
        retained.verify(&original_root, &hash(21)),
        Ok(Some(hash(121)))
    );
    assert_eq!(
        retained.verify(&map.root(), &hash(21)),
        Err(MerkleMapReadError::RootMismatch)
    );
}

#[test]
fn compressed_prefix_and_leaf_divergence_prove_absence() {
    let mut map = MerkleMap::new(&iroha_allocation::AllocationBudget::new(64 * 1024 * 1024));
    let first = Hash::prehashed([0; Hash::LENGTH]);
    let mut bytes = [0; Hash::LENGTH];
    bytes[0] = 0x40;
    map.replace(first, None, Some(hash(1))).unwrap();
    map.replace(Hash::prehashed(bytes), None, Some(hash(2)))
        .unwrap();
    bytes[0] = 0x80;
    let divergent = Hash::prehashed(bytes);
    let proof = map.prove_lookup(&divergent);
    assert_eq!(proof.used, 1);
    assert_eq!(proof.verify(&map.root(), &divergent), Ok(None));

    let mut singleton = MerkleMap::new(&iroha_allocation::AllocationBudget::new(64 * 1024 * 1024));
    singleton.replace(first, None, Some(hash(3))).unwrap();
    let proof = singleton.prove_lookup(&divergent);
    assert_eq!(proof.used, 1);
    assert_eq!(proof.verify(&singleton.root(), &divergent), Ok(None));
}

#[test]
fn forged_or_omitted_path_cannot_prove_absence() {
    let mut map = MerkleMap::new(&iroha_allocation::AllocationBudget::new(64 * 1024 * 1024));
    for n in 0..8 {
        map.replace(hash(n), None, Some(hash(n + 100))).unwrap();
    }
    let key = hash(3);
    let proof = map.prove_lookup(&key);
    assert!(proof.used > 1);
    let root = map.root();
    assert_eq!(proof.verify(&root, &key), Ok(Some(hash(103))));

    let mut missing = proof;
    missing.nodes[1] = None;
    assert!(matches!(
        missing.verify(&root, &key),
        Err(MerkleMapReadError::MissingNode(_))
    ));

    let mut altered_value = proof;
    let terminal = usize::from(altered_value.used - 1);
    let MerkleMapNode::Leaf { value, .. } = altered_value.nodes[terminal].as_mut().unwrap() else {
        panic!("a successful path ends in a leaf")
    };
    value.hash = hash(999);
    assert!(matches!(
        altered_value.verify(&root, &key),
        Err(MerkleMapReadError::NodeHashMismatch(_))
    ));

    let mut forged_branch = proof;
    let MerkleMapNode::Branch { left, .. } = forged_branch.nodes[0].as_mut().unwrap() else {
        panic!("eight rows have a branch root")
    };
    left.hash = hash(999);
    assert!(matches!(
        forged_branch.verify(&root, &key),
        Err(MerkleMapReadError::NodeHashMismatch(_))
    ));

    let mut trailing = proof;
    trailing.nodes[usize::from(trailing.used)] = trailing.nodes[0];
    assert_eq!(
        trailing.verify(&root, &key),
        Err(MerkleMapReadError::InvalidPath)
    );

    let mut overclaimed = proof;
    overclaimed.used += 1;
    assert_eq!(
        overclaimed.verify(&root, &key),
        Err(MerkleMapReadError::InvalidPath)
    );

    let mut wrong_count = proof;
    wrong_count.entries += 1;
    assert_eq!(
        wrong_count.verify(&root, &key),
        Err(MerkleMapReadError::RootMismatch)
    );

    let mut wrong_top = proof;
    wrong_top.top = Some(hash(999));
    assert_eq!(
        wrong_top.verify(&root, &key),
        Err(MerkleMapReadError::RootMismatch)
    );
}
