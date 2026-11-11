//! Persistent update, range completeness, point disclosure and tamper checks.
use super::*;
use std::collections::BTreeMap;

fn k(table: &str, raw: &[u8]) -> StateKeyV1 {
    StateKeyV1 {
        table: table.into(),
        key: raw.into(),
    }
}
fn make() -> KeyedStateTreeV1 {
    KeyedStateTreeV1::new([9; 32], &AllocationBudget::new(4 << 20))
}

#[test]
fn point_proofs_bind_schema_key_presence_and_every_bit_of_the_value() {
    let mut tree = make();
    let a = k("world.assets", &[3]);
    assert!(
        tree.prove(&a)
            .unwrap()
            .verify(&tree.schema, &tree.root(), &a, None)
    );
    tree.replace(&a, Some(b"")).unwrap();
    let unchanged = tree.clone();
    tree.replace(&k("world.assets", &[8]), Some(b"unrelated original"))
        .unwrap();
    tree.replace(&k("world.cell", &[]), Some(b"cell")).unwrap();
    let root = tree.root();
    let proof = tree.prove(&a).unwrap();
    assert_eq!(
        tree.proof_heap_bytes(&a).unwrap(),
        proof.steps.len() * std::mem::size_of::<Step>()
    );
    assert_eq!(tree.schema_digest(), tree.schema);
    let bytes = norito::to_bytes(&proof).unwrap();
    let decoded: StateLookupProofV1 = norito::decode_from_bytes(&bytes).unwrap();
    assert_eq!(proof, decoded);
    assert!(proof.verify(&tree.schema, &root, &a, Some(b"")));
    assert!(!proof.verify(&tree.schema, &root, &a, None));
    assert!(!proof.verify(&tree.schema, &root, &a, Some(&[0])));
    assert!(!proof.verify(&[2; 32], &root, &a, Some(b"")));
    assert!(!proof.verify(&tree.schema, &unchanged.root(), &a, Some(b"")));
    let missing = k("world.assets", &[0]);
    let absence = tree.prove(&missing).unwrap();
    assert!(absence.verify(&tree.schema, &root, &missing, None));
    assert!(!absence.verify(&tree.schema, &root, &a, None));
    let mut altered = absence;
    altered.steps[0].sibling.count += 1;
    assert!(!altered.verify(&tree.schema, &root, &missing, None));
    assert_eq!(unchanged.get(&a), Some(&[][..]));
    assert_eq!(unchanged.len(), 1);
}

#[test]
fn incremental_insert_delete_reinsert_matches_cold_build_in_both_orders() {
    let mut tree = make();
    let mut model = BTreeMap::new();
    for n in 0..300u32 {
        let raw = (n.wrapping_mul(7_919) % 97).to_be_bytes();
        let key = k(if n % 3 == 0 { "world.a" } else { "world.aa" }, &raw);
        let v = n.to_le_bytes().to_vec();
        if n % 4 == 0 {
            tree.replace(&key, None).unwrap();
            model.remove(&key);
        } else {
            tree.replace(&key, Some(&v)).unwrap();
            model.insert(key, v);
        }
        if n % 17 == 0 {
            let mut cold = make();
            for (key, value) in model.iter().rev() {
                cold.replace(key, Some(value)).unwrap();
            }
            assert_eq!(tree.root(), cold.root());
            assert_eq!(tree.len(), model.len() as u64);
        }
    }
    for key in model.keys() {
        tree.replace(key, None).unwrap();
    }
    assert!(tree.is_empty());
    assert_eq!(tree.root(), make().root());
}

#[test]
fn range_proofs_authenticate_consecutive_positions_and_neighbors_across_tables() {
    let mut tree = make();
    let rows = [
        (&b""[..], &b"x"[..]),
        (&b"a"[..], &b"y"[..]),
        (&b"a\0"[..], &b""[..]),
        (&b"z"[..], &b"z"[..]),
    ];
    for (key, v) in rows {
        tree.replace(&k("world.ab", key), Some(v)).unwrap();
    }
    tree.replace(&k("world.a", b""), Some(b"A")).unwrap();
    tree.replace(&k("world.ac", b""), Some(b"C")).unwrap();
    let root = tree.root();
    let proof = tree.prove_range("world.ab", b"a", Some(b"z"), 2).unwrap();
    assert!(proof.verify(
        &tree.schema,
        &root,
        "world.ab",
        b"a",
        Some(b"z"),
        &rows[1..3]
    ));
    assert!(!proof.verify(
        &tree.schema,
        &root,
        "world.ab",
        b"a",
        Some(b"z"),
        &rows[2..3]
    ));
    let mut omitted = proof.clone();
    omitted.rows.remove(0);
    assert!(!omitted.verify(
        &tree.schema,
        &root,
        "world.ab",
        b"a",
        Some(b"z"),
        &rows[2..3]
    ));
    let mut missing_suffix = proof.clone();
    missing_suffix.after = None;
    assert!(!missing_suffix.verify(
        &tree.schema,
        &root,
        "world.ab",
        b"a",
        Some(b"z"),
        &rows[1..3]
    ));
    assert!(tree.prove_range("world.ab", b"", None, 3).is_err());
    assert!(tree.prove_range("world.ab", b"", None, 4).unwrap().verify(
        &tree.schema,
        &root,
        "world.ab",
        b"",
        None,
        &rows
    ));
    assert!(
        tree.prove_range("world.ab", b"aa", Some(b"b"), 0)
            .unwrap()
            .verify(&tree.schema, &root, "world.ab", b"aa", Some(b"b"), &[])
    );
    assert!(tree.prove_range("world.z", b"", None, 0).unwrap().verify(
        &tree.schema,
        &root,
        "world.z",
        b"",
        None,
        &[]
    ));
}

#[test]
fn local_refusal_preserves_root_and_releases_every_staged_allocation() {
    let pool = AllocationBudget::new(4096);
    let mut tree = KeyedStateTreeV1::new([3; 32], &pool);
    tree.replace(&k("world.a", b"a"), Some(b"ok")).unwrap();
    let root = tree.root();
    let held = pool.reserved_bytes();
    assert!(tree.replace(&k("world.a", b"b"), Some(&[8; 4096])).is_err());
    assert_eq!(pool.reserved_bytes(), held);
    assert_eq!(tree.root(), root);
    assert!(tree.replace(&k("world.\0bad", b"b"), Some(b"bad")).is_err());
    drop(tree);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn streaming_cold_build_matches_incremental_and_rejects_out_of_order() {
    let mut hot = make();
    let mut builder = KeyedStateBuilderV1::new(hot.schema, &hot.budget);
    let mut inputs = vec![];
    for table in ["world.a", "world.aa", "world.b"] {
        for byte in 0..48 {
            inputs.push(k(table, &[byte / 8, byte]));
        }
    }
    for key in &inputs {
        builder.push(key, &key.key).unwrap();
        assert!(builder.push(key, &key.key).is_err());
    }
    for key in inputs.iter().rev() {
        hot.replace(key, Some(&key.key)).unwrap();
    }
    let cold = builder.finish().unwrap();
    assert_eq!(cold.root(), hot.root());
    for key in &inputs {
        assert_eq!(hot.prove(key).unwrap(), cold.prove(key).unwrap());
    }
}

#[test]
fn long_adversarial_prefixes_use_the_funded_deep_path() {
    let mut tree = make();
    let mut builder = KeyedStateBuilderV1::new(tree.schema, &tree.budget);
    for len in 0..180 {
        let key = k("world.a", &vec![0xff; len]);
        tree.replace(&key, Some(&[])).unwrap();
        builder.push(&key, &[]).unwrap();
    }
    assert_eq!(tree.root(), builder.finish().unwrap().root());
    let last = k("world.a", &[0xff; 179]);
    assert!(tree.prove(&last).unwrap().steps.len() > 32);
    tree.replace(&last, Some(&[7])).unwrap();
    assert!(
        tree.prove(&last)
            .unwrap()
            .verify(&tree.schema, &tree.root(), &last, Some(&[7]))
    );
    tree.replace(&last, None).unwrap();
    assert!(
        tree.prove(&last)
            .unwrap()
            .verify(&tree.schema, &tree.root(), &last, None)
    );
}

#[test]
fn raw_blake3_golden_roots_pin_byte_order_and_the_unmarked_empty_root() {
    let mut tree = KeyedStateTreeV1::new([0x5a; 32], &AllocationBudget::new(32 << 10));
    let hex = |root: [u8; 32]| hex::encode(root);
    assert_eq!(
        hex(tree.root()),
        "2c70240ea48b7696d0cd4d407fcf5cf12d2fbe4f7f55eba4895e90b74225097a"
    );
    // The prefix tree splits at bit 19 for 00 vs ff; bit 27 is the shorter
    // key's end marker. The vector was computed directly from those two branches.
    for (key, val) in [
        (&[0xff][..], &b"Z"[..]),
        (&[0, 0][..], &b"abc"[..]),
        (&[0][..], &b""[..]),
    ] {
        tree.replace(&k("t", key), Some(val)).unwrap();
    }
    assert_eq!(
        hex(tree.root()),
        "1c0fc81f5220f4a95a20e6b74ecade19cbf94e0d3f62a47a7f8be17cceb09b83"
    );
}
