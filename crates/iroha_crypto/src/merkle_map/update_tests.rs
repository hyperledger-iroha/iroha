//! Canonical external edits, original-version custody and every I/O failure cut.

use super::*;
use crate::MerkleMap;
use std::collections::BTreeMap;

type Location = u128;
type LocatedNode = MerkleMapNode<Location, Location>;
type LocatedRoot = MerkleMapRoot<Location>;
type Workspace = MerkleMapUpdateWorkspace<Location, Location>;
type Edit = MerkleMapEdit<Location>;

fn located_value(hash: Hash) -> MerkleMapValueRef<Location> {
    MerkleMapValueRef {
        hash,
        location: u128::from_le_bytes(hash.as_ref()[..16].try_into().unwrap()),
    }
}

fn hash(n: u64) -> Hash {
    Hash::new(n.to_le_bytes())
}

#[derive(Clone, Default)]
struct Store {
    nodes: BTreeMap<Location, LocatedNode>,
    next: Location,
    reads: usize,
    writes: usize,
    fail_read: Option<usize>,
    fail_write: Option<usize>,
    fail_after_write: bool,
    panic_write: bool,
}

impl MerkleMapNodeStore for Store {
    type NodeLocation = Location;
    type ValueLocation = Location;
    type Error = &'static str;

    fn read(
        &mut self,
        reference: &MerkleMapNodeRef<Location>,
    ) -> Result<Option<LocatedNode>, Self::Error> {
        self.reads += 1;
        if self.fail_read == Some(self.reads) {
            return Err("read refused");
        }
        Ok(self.nodes.get(&reference.location).copied())
    }

    fn write(&mut self, node: LocatedNode) -> Result<Location, Self::Error> {
        self.writes += 1;
        let fail = self.fail_write == Some(self.writes);
        if fail && !self.fail_after_write {
            return Err("write refused");
        }
        if let LocatedNode::Branch { left, right, .. } = node {
            assert!(
                self.nodes.contains_key(&left.location),
                "left child precedes parent"
            );
            assert!(
                self.nodes.contains_key(&right.location),
                "right child precedes parent"
            );
        }
        self.next += 1;
        assert!(
            self.nodes.insert(self.next, node).is_none(),
            "physical bindings are immutable"
        );
        if fail {
            assert!(!self.panic_write, "store unwound after its write");
            return Err("write refused");
        }
        Ok(self.next)
    }
}

fn export(map: &MerkleMap) -> (Store, LocatedRoot) {
    let mut store = Store::default();
    let root = map
        .export_nodes(&mut store, |_, hash| Ok(located_value(hash).location))
        .unwrap();
    store.writes = 0;
    (store, root)
}

fn read(root: LocatedRoot, key: Hash, store: &Store) -> Option<Hash> {
    root.lookup(&root.hash(), &key, |reference| {
        Ok::<_, ()>(store.nodes.get(&reference.location).copied())
    })
    .unwrap()
    .map(|value| value.hash)
}

fn apply(
    root: LocatedRoot,
    edit: Edit,
    workspace: &mut Workspace,
    store: &mut Store,
) -> LocatedRoot {
    store.reads = 0;
    store.writes = 0;
    let result = root.replace(&root.hash(), edit, workspace, store).unwrap();
    assert!(store.reads <= 257);
    assert!(store.writes <= 257);
    if edit.expected == edit.after.map(|value| value.hash) {
        assert_eq!(store.writes, 0);
        assert_eq!(result, root);
    }
    result
}

#[test]
fn external_edits_match_canonical_rebuilds_and_retain_every_old_version() {
    let mut map = MerkleMap::new();
    let mut store = Store::default();
    let mut root = LocatedRoot::from_parts(0, None);
    let mut workspace = Workspace::default();
    let mut entries = BTreeMap::new();
    let mut snapshots = Vec::new();
    for step in 0..320 {
        let key = hash(step % 43);
        let expected = entries.get(&key).copied();
        let after = match step % 5 {
            0 => expected,
            1 => None,
            _ => Some(hash(step + 1000)),
        };
        let original = root;
        root = apply(
            root,
            Edit {
                key,
                expected,
                after: after.map(located_value),
            },
            &mut workspace,
            &mut store,
        );
        map.replace(key, expected, after).unwrap();
        assert_eq!(root.hash(), map.root());
        assert_eq!(read(original, key, &store), expected);
        if let Some(value) = after {
            entries.insert(key, value);
        } else {
            entries.remove(&key);
        }
        let mut cold = MerkleMap::new();
        for (&key, &value) in entries.iter().rev() {
            cold.replace(key, None, Some(value)).unwrap();
        }
        assert_eq!(root.hash(), cold.root(), "cold reverse-order rebuild");
        snapshots.push((root, entries.clone()));
    }
    drop(map);
    for (root, expected) in snapshots {
        for n in 0..47 {
            assert_eq!(read(root, hash(n), &store), expected.get(&hash(n)).copied());
        }
    }
}

#[test]
fn deleting_all_keys_in_different_orders_collapses_to_the_original_empty_root() {
    let empty = LocatedRoot::from_parts(0, None);
    let mut workspace = Workspace::new();
    for reverse in [false, true] {
        let mut map = MerkleMap::new();
        let mut store = Store::default();
        let mut root = empty;
        for n in 0..80 {
            let key = hash(n);
            let value = hash(n + 100);
            root = apply(
                root,
                Edit {
                    key,
                    expected: None,
                    after: Some(located_value(value)),
                },
                &mut workspace,
                &mut store,
            );
            map.replace(key, None, Some(value)).unwrap();
            assert_eq!(root.hash(), map.root());
        }
        let original = root;
        for i in 0..80 {
            let n = if reverse { 79 - i } else { i };
            let key = hash(n);
            root = apply(
                root,
                Edit {
                    key,
                    expected: Some(hash(n + 100)),
                    after: None,
                },
                &mut workspace,
                &mut store,
            );
            map.replace(key, Some(hash(n + 100)), None).unwrap();
            assert_eq!(root.hash(), map.root());
            assert_eq!(read(original, key, &store), Some(hash(n + 100)));
        }
        assert_eq!(root, empty);
        assert_eq!(store.writes, 0, "last leaf removal writes no nodes");
        // Reusing a long-path workspace with an empty root cannot reuse old nodes.
        root = apply(
            root,
            Edit {
                key: hash(999),
                expected: None,
                after: Some(located_value(hash(1000))),
            },
            &mut workspace,
            &mut store,
        );
        assert_eq!((store.reads, store.writes), (0, 1));
        assert_eq!(read(root, hash(999), &store), Some(hash(1000)));
    }
}

#[test]
fn every_read_and_write_failure_retains_the_original_root_and_all_bindings() {
    let mut map = MerkleMap::new();
    for n in 0..128 {
        map.replace(hash(n), None, Some(hash(n + 500))).unwrap();
    }
    let (original, root) = export(&map);
    let mut workspace = Workspace::new();
    for edit in [
        Edit {
            key: hash(19),
            expected: Some(hash(519)),
            after: Some(located_value(hash(999))),
        },
        Edit {
            key: hash(19),
            expected: Some(hash(519)),
            after: None,
        },
        Edit {
            key: hash(999),
            expected: None,
            after: Some(located_value(hash(1000))),
        },
    ] {
        let mut good = original.clone();
        let successor = apply(root, edit, &mut workspace, &mut good);
        for failure in 1..=good.reads {
            let mut failed = original.clone();
            failed.fail_read = Some(failure);
            assert_eq!(
                root.replace(&root.hash(), edit, &mut workspace, &mut failed),
                Err(MerkleMapUpdateError::Read(MerkleMapReadError::Source(
                    "read refused"
                )))
            );
            assert_eq!(failed.writes, 0);
            assert_eq!(failed.nodes, original.nodes);
        }
        for after_write in [false, true] {
            for failure in 1..=good.writes {
                let mut failed = original.clone();
                failed.fail_write = Some(failure);
                failed.fail_after_write = after_write;
                assert_eq!(
                    root.replace(&root.hash(), edit, &mut workspace, &mut failed),
                    Err(MerkleMapUpdateError::Write("write refused"))
                );
                assert_eq!(failed.writes, failure);
                for (reference, node) in &original.nodes {
                    assert_eq!(failed.nodes.get(reference), Some(node));
                }
                for n in 0..130 {
                    assert_eq!(read(root, hash(n), &failed), map.get(&hash(n)));
                }
                failed.fail_write = None;
                assert_eq!(
                    apply(root, edit, &mut workspace, &mut failed).hash(),
                    successor.hash(),
                    "same original version retries after partial writes"
                );
            }
        }
        if good.writes > 0 {
            let mut failed = original.clone();
            failed.fail_write = Some(good.writes);
            failed.fail_after_write = true;
            failed.panic_write = true;
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    root.replace(&root.hash(), edit, &mut workspace, &mut failed)
                }))
                .is_err()
            );
            assert_eq!(read(root, edit.key, &failed), edit.expected);
            failed.fail_write = None;
            assert_eq!(
                apply(root, edit, &mut workspace, &mut failed).hash(),
                successor.hash()
            );
        }
    }
}

#[test]
fn invalid_authority_preimages_counts_and_storage_fail_before_any_write() {
    let mut map = MerkleMap::new();
    for n in 0..4 {
        map.replace(hash(n), None, Some(hash(n + 20))).unwrap();
    }
    let (mut store, root) = export(&map);
    let mut workspace = Workspace::new();
    let edit = Edit {
        key: hash(0),
        expected: Some(hash(20)),
        after: Some(located_value(hash(30))),
    };
    assert_eq!(
        root.replace(&hash(404), edit, &mut workspace, &mut store),
        Err(MerkleMapUpdateError::Read(MerkleMapReadError::RootMismatch))
    );
    assert_eq!((store.reads, store.writes), (0, 0));
    for (key, expected, actual) in [
        (hash(0), None, Some(hash(20))),
        (hash(999), Some(hash(20)), None),
    ] {
        assert_eq!(
            root.replace(
                &root.hash(),
                Edit {
                    key,
                    expected,
                    after: expected.map(located_value)
                },
                &mut workspace,
                &mut store
            ),
            Err(MerkleMapUpdateError::Edit(
                MerkleMapError::PreimageMismatch { expected, actual }
            ))
        );
    }
    let overflow = LocatedRoot::from_parts(u64::MAX, root.parts().1);
    assert_eq!(
        overflow.replace(
            &overflow.hash(),
            Edit {
                key: hash(999),
                expected: None,
                after: Some(located_value(hash(20)))
            },
            &mut workspace,
            &mut store
        ),
        Err(MerkleMapUpdateError::Edit(MerkleMapError::Capacity))
    );
    let top = root.parts().1.unwrap();
    store.nodes.remove(&top.location);
    assert_eq!(
        root.replace(&root.hash(), edit, &mut workspace, &mut store),
        Err(MerkleMapUpdateError::Read(MerkleMapReadError::MissingNode(
            top.hash
        )))
    );
    store.nodes.insert(
        top.location,
        LocatedNode::Leaf {
            key: edit.key,
            value: located_value(hash(1234)),
        },
    );
    assert_eq!(
        root.replace(&root.hash(), edit, &mut workspace, &mut store),
        Err(MerkleMapUpdateError::Read(
            MerkleMapReadError::NodeHashMismatch(top.hash)
        ))
    );
    assert_eq!(store.writes, 0);
}

#[test]
fn inserts_split_leaves_and_compressed_prefixes_at_every_valid_bit() {
    let mut workspace = Workspace::new();
    for reverse in [false, true] {
        let mut map = MerkleMap::new();
        let base = Hash::prehashed([0; 32]);
        map.replace(base, None, Some(hash(999))).unwrap();
        let (mut store, mut root) = export(&map);
        for index in 0..255 {
            let bit = if reverse { 254 - index } else { index };
            let mut bytes = [0; 32];
            bytes[bit / 8] = 128 >> (bit % 8);
            let key = Hash::prehashed(bytes);
            let value = hash(bit as u64);
            let original = root;
            root = apply(
                root,
                Edit {
                    key,
                    expected: None,
                    after: Some(located_value(value)),
                },
                &mut workspace,
                &mut store,
            );
            map.replace(key, None, Some(value)).unwrap();
            assert_eq!(root.hash(), map.root());
            assert_eq!(read(original, key, &store), None);
            assert_eq!(read(root, key, &store), Some(value));
            assert_eq!(read(root, base, &store), Some(hash(999)));
            assert_eq!(store.reads, if reverse { 1 } else { index + 1 });
            assert_eq!(store.writes, if reverse { 2 } else { index + 2 });
        }
    }
}

#[test]
fn deepest_updates_have_fixed_work_and_fit_the_default_thread_stack() {
    std::thread::spawn(|| {
        use zeroize::Zeroize;
        let mut map = MerkleMap::new();
        let base = Hash::prehashed([0; 32]);
        map.replace(base, None, Some(hash(999))).unwrap();
        for bit in 0..255 {
            let mut bytes = [0; 32];
            bytes[bit / 8] = 128 >> (bit % 8);
            map.replace(Hash::prehashed(bytes), None, Some(hash(bit as u64)))
                .unwrap();
        }
        let (mut store, original) = export(&map);
        let mut root = original;
        let mut workspace = Workspace::new();
        // Hash's normal marker excludes bit-255 splits, but zeroization can
        // produce this in-memory input. Exercise the full advertised bound.
        let mut key = base;
        key.zeroize();
        for (expected, after, reads, writes) in [
            (None, Some(hash(1000)), 256, 257),
            (Some(hash(1000)), Some(hash(1001)), 257, 257),
            (Some(hash(1001)), Some(hash(1001)), 257, 0),
            (Some(hash(1001)), None, 257, 255),
        ] {
            root = apply(
                root,
                Edit {
                    key,
                    expected,
                    after: after.map(located_value),
                },
                &mut workspace,
                &mut store,
            );
            assert_eq!((store.reads, store.writes), (reads, writes));
            map.replace(key, expected, after).unwrap();
            assert_eq!(root.hash(), map.root());
            assert_eq!(read(root, key, &store), after);
            assert_eq!(read(original, base, &store), Some(hash(999)));
        }
        assert_eq!(root.hash(), original.hash());
    })
    .join()
    .unwrap();
}

#[test]
fn identical_value_at_another_location_is_an_exact_original_root_noop() {
    let mut map = MerkleMap::new();
    map.replace(hash(1), None, Some(hash(2))).unwrap();
    let (mut store, root) = export(&map);
    let old = located_value(hash(2));
    let relocated = MerkleMapValueRef {
        hash: old.hash,
        location: old.location ^ 1,
    };
    let mut workspace = Workspace::new();
    let result = apply(
        root,
        Edit {
            key: hash(1),
            expected: Some(old.hash),
            after: Some(relocated),
        },
        &mut workspace,
        &mut store,
    );
    assert_eq!(result, root);
    assert_eq!(store.writes, 0);
    assert_eq!(
        result
            .lookup(&root.hash(), &hash(1), |reference| store.read(reference))
            .unwrap(),
        Some(old)
    );
}

#[test]
fn divergent_insertion_and_deletion_retain_the_exact_untouched_subtree_location() {
    let first = Hash::prehashed([0; 32]);
    let mut bytes = [0; 32];
    bytes[0] = 0x40;
    let second = Hash::prehashed(bytes);
    bytes[0] = 0x80;
    let added = Hash::prehashed(bytes);
    let mut map = MerkleMap::new();
    map.replace(first, None, Some(hash(1))).unwrap();
    map.replace(second, None, Some(hash(2))).unwrap();
    let (mut store, original) = export(&map);
    let top = original.parts().1.unwrap();
    let original_node = store.nodes[&top.location];
    let mut workspace = Workspace::new();
    let expanded = apply(
        original,
        Edit {
            key: added,
            expected: None,
            after: Some(located_value(hash(3))),
        },
        &mut workspace,
        &mut store,
    );
    assert_eq!((store.reads, store.writes), (1, 2));
    let LocatedNode::Branch { left, .. } = store.nodes[&expanded.parts().1.unwrap().location]
    else {
        panic!("new divergent branch");
    };
    assert_eq!(
        left, top,
        "reuse the original terminal reference, not its hash as a location"
    );
    assert_eq!(store.nodes[&top.location], original_node);
    let collapsed = apply(
        expanded,
        Edit {
            key: added,
            expected: Some(hash(3)),
            after: None,
        },
        &mut workspace,
        &mut store,
    );
    assert_eq!(
        (store.reads, store.writes),
        (2, 0),
        "no untouched sibling read or write"
    );
    assert_eq!(collapsed, original);
    assert_eq!(read(expanded, added, &store), Some(hash(3)));
}
