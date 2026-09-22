//! External-node authentication, genuine absence and bounded traversal controls.

use super::*;
use std::{collections::BTreeMap, convert::Infallible};

fn hash(n: u64) -> Hash {
    Hash::new(n.to_le_bytes())
}

type Location = u128;
type LocatedNode = MerkleMapNode<Location, Location>;
type LocatedRoot = MerkleMapRoot<Location>;

fn value(hash: Hash) -> MerkleMapValueRef<Location> {
    MerkleMapValueRef {
        hash,
        location: u128::from_le_bytes(hash.as_ref()[..16].try_into().unwrap()),
    }
}
fn reference(hash: Hash) -> MerkleMapNodeRef<Location> {
    MerkleMapNodeRef {
        hash,
        location: value(hash).location,
    }
}

#[derive(Default)]
struct Store {
    nodes: BTreeMap<Location, LocatedNode>,
    next: Location,
    writes: usize,
    fail_write: Option<usize>,
}
impl MerkleMapNodeStore for Store {
    type NodeLocation = Location;
    type ValueLocation = Location;
    type Error = &'static str;
    fn read(
        &mut self,
        node: &MerkleMapNodeRef<Location>,
    ) -> Result<Option<LocatedNode>, Self::Error> {
        Ok(self.nodes.get(&node.location).copied())
    }
    fn write(&mut self, node: LocatedNode) -> Result<Location, Self::Error> {
        self.writes += 1;
        if self.fail_write == Some(self.writes) {
            return Err("admission refused");
        }
        if let LocatedNode::Branch { left, right, .. } = node {
            assert_eq!(
                self.nodes.get(&left.location).unwrap().hash(),
                left.hash,
                "children precede their parent"
            );
            assert_eq!(self.nodes.get(&right.location).unwrap().hash(), right.hash);
        }
        self.next += 1;
        assert!(
            self.nodes.insert(self.next, node).is_none(),
            "immutable physical locations"
        );
        Ok(self.next)
    }
}

fn export(map: &MerkleMap, store: &mut Store) -> LocatedRoot {
    let root = map
        .export_nodes(store, |_, hash| Ok(value(hash).location))
        .unwrap();
    assert_eq!(root.hash(), map.root());
    assert_eq!(root.parts().0, map.len());
    root
}

fn read(root: &LocatedRoot, key: Hash, store: &Store) -> Option<Hash> {
    root.lookup::<Location, _>(&root.hash(), &key, |reference| {
        Ok::<_, Infallible>(store.nodes.get(&reference.location).copied())
    })
    .unwrap()
    .map(|value| value.hash)
}

#[test]
fn external_versions_match_actual_mutations_and_survive_resident_drop() {
    let mut map = MerkleMap::new();
    let mut store = Store::default();
    let empty = export(&map, &mut store);
    assert_eq!(read(&empty, hash(0), &store), None);
    let mut retained = Vec::new();
    let mut expected = BTreeMap::new();
    for step in 0..240 {
        let key = hash(step % 37);
        let before = expected.get(&key).copied();
        let after = (step % 4 != 0).then(|| hash(step + 1000));
        map.replace(key, before, after).unwrap();
        if let Some(value) = after {
            expected.insert(key, value);
        } else {
            expected.remove(&key);
        }
        let root = export(&map, &mut store);
        for n in 0..42 {
            assert_eq!(
                read(&root, hash(n), &store),
                expected.get(&hash(n)).copied()
            );
        }
        if step % 30 == 0 {
            retained.push((root, expected.clone()));
        }
    }
    drop(map);
    for (root, expected) in retained {
        for n in 0..42 {
            assert_eq!(
                read(&root, hash(n), &store),
                expected.get(&hash(n)).copied()
            );
        }
    }
}

#[test]
fn root_authentication_and_metadata_fail_before_external_reads() {
    let mut map = MerkleMap::new();
    map.replace(hash(1), None, Some(hash(2))).unwrap();
    let root = export(&map, &mut Store::default());
    let (len, node) = root.parts();
    for invalid in [
        LocatedRoot::from_parts(len + 1, node),
        LocatedRoot::from_parts(len, None),
    ] {
        assert_eq!(
            invalid.lookup::<Location, _>(
                &root.hash(),
                &hash(1),
                |_| -> Result<Option<LocatedNode>, ()> { panic!("untrusted root must not read") }
            ),
            Err(MerkleMapReadError::RootMismatch)
        );
    }
    for invalid in [
        LocatedRoot::from_parts(0, node),
        LocatedRoot::from_parts(1, None),
    ] {
        assert_eq!(
            invalid.lookup::<Location, _>(
                &invalid.hash(),
                &hash(1),
                |_| -> Result<Option<LocatedNode>, ()> { panic!("invalid shape must not read") }
            ),
            Err(MerkleMapReadError::InvalidRoot)
        );
    }
    let empty = export(&MerkleMap::new(), &mut Store::default());
    assert_eq!(
        empty.lookup::<Location, _>(
            &empty.hash(),
            &hash(1),
            |_| -> Result<Option<LocatedNode>, ()> { panic!("empty map must not read") }
        ),
        Ok(None)
    );
}

#[test]
fn missing_source_and_corrupt_content_never_become_absence() {
    let mut map = MerkleMap::new();
    for n in 0..8 {
        map.replace(hash(n), None, Some(hash(n + 20))).unwrap();
    }
    let mut store = Store::default();
    let root = export(&map, &mut store);
    let top = root.parts().1.unwrap();
    assert_eq!(
        root.lookup::<Location, _>(&root.hash(), &hash(0), |_| Ok::<_, ()>(None)),
        Err(MerkleMapReadError::MissingNode(top.hash))
    );
    assert_eq!(
        root.lookup::<Location, _>(&root.hash(), &hash(0), |_| Err::<Option<LocatedNode>, _>(
            "disk failure"
        )),
        Err(MerkleMapReadError::Source("disk failure"))
    );
    assert_eq!(
        root.lookup::<Location, _>(&root.hash(), &hash(0), |_| Ok::<_, ()>(Some(
            LocatedNode::Leaf {
                key: hash(0),
                value: value(hash(999))
            }
        ))),
        Err(MerkleMapReadError::NodeHashMismatch(top.hash))
    );
    let mut calls = 0;
    let mut missing = None;
    let result = root.lookup::<Location, _>(&root.hash(), &hash(0), |reference| {
        calls += 1;
        if calls == 2 {
            missing = Some(*reference);
            Ok::<_, ()>(None)
        } else {
            Ok(store.nodes.get(&reference.location).copied())
        }
    });
    assert_eq!(calls, 2);
    assert_eq!(
        result,
        Err(MerkleMapReadError::MissingNode(missing.unwrap().hash))
    );
}

#[test]
fn authenticated_prefix_and_leaf_divergence_prove_absence() {
    let mut map = MerkleMap::new();
    let first = Hash::prehashed([0; 32]);
    let mut bytes = [0; 32];
    bytes[0] = 0x40;
    map.replace(first, None, Some(hash(1))).unwrap();
    map.replace(Hash::prehashed(bytes), None, Some(hash(2)))
        .unwrap();
    let mut store = Store::default();
    let root = export(&map, &mut store);
    bytes[0] = 0x80;
    let mut calls = 0;
    assert_eq!(
        root.lookup::<Location, _>(&root.hash(), &Hash::prehashed(bytes), |reference| {
            calls += 1;
            Ok::<_, ()>(store.nodes.get(&reference.location).copied())
        }),
        Ok(None)
    );
    assert_eq!(calls, 1, "divergence at authenticated compressed prefix");
    let mut singleton = MerkleMap::new();
    singleton.replace(first, None, Some(hash(3))).unwrap();
    let root = export(&singleton, &mut store);
    assert_eq!(read(&root, Hash::prehashed(bytes), &store), None);
}

fn malformed_result(
    len: u64,
    root_node: LocatedNode,
    others: &[LocatedNode],
    key: Hash,
) -> Result<Option<MerkleMapValueRef<Location>>, MerkleMapReadError<()>> {
    let mut store = others
        .iter()
        .map(|node| (reference(node.hash()).location, *node))
        .collect::<BTreeMap<_, _>>();
    let reference = reference(root_node.hash());
    store.insert(reference.location, root_node);
    let root = LocatedRoot::from_parts(len, Some(reference));
    root.lookup::<Location, _>(&root.hash(), &key, |reference| {
        Ok(store.get(&reference.location).copied())
    })
}

#[test]
fn malformed_depth_prefix_and_root_arity_are_rejected() {
    let key = Hash::prehashed([0; 32]);
    let leaf = LocatedNode::Leaf {
        key,
        value: value(hash(3)),
    };
    assert_eq!(
        malformed_result(2, leaf, &[], key),
        Err(MerkleMapReadError::InvalidPath)
    );
    for bit in [256, u16::MAX] {
        let node = LocatedNode::Branch {
            bit,
            prefix: [0; 32],
            left: reference(leaf.hash()),
            right: reference(hash(4)),
        };
        assert_eq!(
            malformed_result(2, node, &[leaf], key),
            Err(MerkleMapReadError::InvalidPath)
        );
    }
    for bit in [0, 1, 7, 8, 254, 255] {
        let mut prefix = [0; 32];
        prefix[31] = 1;
        let node = LocatedNode::Branch {
            bit,
            prefix,
            left: reference(leaf.hash()),
            right: reference(hash(4)),
        };
        assert_eq!(
            malformed_result(2, node, &[leaf], key),
            Err(MerkleMapReadError::InvalidPath)
        );
    }
    let node = LocatedNode::Branch {
        bit: 0,
        prefix: [0; 32],
        left: reference(leaf.hash()),
        right: reference(leaf.hash()),
    };
    assert_eq!(
        malformed_result(2, node, &[leaf], key),
        Err(MerkleMapReadError::InvalidPath)
    );
    let node = LocatedNode::Branch {
        bit: 0,
        prefix: [0; 32],
        left: reference(leaf.hash()),
        right: reference(hash(4)),
    };
    assert_eq!(
        malformed_result(1, node, &[leaf], key),
        Err(MerkleMapReadError::InvalidPath)
    );
}

#[test]
fn independently_hashed_children_must_preserve_original_parent_path() {
    let key = Hash::prehashed([0; 32]);
    let wrong_side = LocatedNode::Leaf {
        key: Hash::prehashed([255; 32]),
        value: value(hash(3)),
    };
    let parent = LocatedNode::Branch {
        bit: 0,
        prefix: [0; 32],
        left: reference(wrong_side.hash()),
        right: reference(hash(4)),
    };
    assert_eq!(
        malformed_result(2, parent, &[wrong_side], key),
        Err(MerkleMapReadError::InvalidPath)
    );
    let repeated_depth = LocatedNode::Branch {
        bit: 0,
        prefix: [0; 32],
        left: reference(hash(3)),
        right: reference(hash(4)),
    };
    let parent = LocatedNode::Branch {
        bit: 0,
        prefix: [0; 32],
        left: reference(repeated_depth.hash()),
        right: reference(hash(5)),
    };
    assert_eq!(
        malformed_result(3, parent, &[repeated_depth], key),
        Err(MerkleMapReadError::InvalidPath)
    );
    let mut prefix = [0; 32];
    prefix[0] = 0x80;
    let wrong_prefix = LocatedNode::Branch {
        bit: 2,
        prefix,
        left: reference(hash(3)),
        right: reference(hash(4)),
    };
    let parent = LocatedNode::Branch {
        bit: 1,
        prefix: [0; 32],
        left: reference(wrong_prefix.hash()),
        right: reference(hash(5)),
    };
    assert_eq!(
        malformed_result(3, parent, &[wrong_prefix], key),
        Err(MerkleMapReadError::InvalidPath)
    );
}

#[test]
fn all_split_bits_export_and_lookup_on_the_default_thread_stack() {
    std::thread::spawn(|| {
        let key = Hash::prehashed([0; 32]);
        let mut map = MerkleMap::new();
        map.replace(key, None, Some(hash(999))).unwrap();
        for bit in (0..255).rev() {
            let mut bytes = [0; 32];
            bytes[bit / 8] = 128 >> (bit % 8);
            map.replace(Hash::prehashed(bytes), None, Some(hash(bit as u64)))
                .unwrap();
        }
        let mut store = Store::default();
        let root = export(&map, &mut store);
        assert_eq!(store.nodes.len(), 511);
        let mut calls = 0;
        assert_eq!(
            root.lookup::<Location, _>(&root.hash(), &key, |reference| {
                calls += 1;
                Ok::<_, ()>(store.nodes.get(&reference.location).copied())
            }),
            Ok(Some(value(hash(999))))
        );
        assert_eq!(calls, 256);
        for bit in 0..255 {
            let mut bytes = [0; 32];
            bytes[bit / 8] = 128 >> (bit % 8);
            assert_eq!(
                read(&root, Hash::prehashed(bytes), &store),
                Some(hash(bit as u64))
            );
        }
    })
    .join()
    .unwrap();
}

#[test]
fn export_stops_at_the_original_error_without_changing_versions() {
    let mut map = MerkleMap::new();
    for n in 0..32 {
        map.replace(hash(n), None, Some(hash(n + 50))).unwrap();
    }
    let original = map.root();
    let mut failed = Store {
        fail_write: Some(7),
        ..Store::default()
    };
    assert_eq!(
        map.export_nodes(&mut failed, |_, hash| Ok(value(hash).location)),
        Err("admission refused")
    );
    assert_eq!(failed.writes, 7);
    assert_eq!(map.root(), original);
    let mut store = Store::default();
    let root = export(&map, &mut store);
    assert_eq!(read(&root, hash(9), &store), Some(hash(59)));
}

#[test]
fn physical_node_and_value_placement_never_changes_logical_commitments() {
    let mut map = MerkleMap::new();
    for n in 0..32 {
        map.replace(hash(n), None, Some(hash(n + 100))).unwrap();
    }
    let mut first = Store::default();
    let a = export(&map, &mut first);
    let mut second = Store {
        next: 1 << 64,
        ..Store::default()
    };
    let b = map
        .export_nodes(&mut second, |_, hash| Ok(value(hash).location ^ 1))
        .unwrap();
    assert_eq!(a.hash(), b.hash());
    assert_ne!(a.parts().1.unwrap().location, b.parts().1.unwrap().location);
    for n in 0..32 {
        let av = a
            .lookup(&a.hash(), &hash(n), |reference| first.read(reference))
            .unwrap()
            .unwrap();
        let bv = b
            .lookup(&b.hash(), &hash(n), |reference| second.read(reference))
            .unwrap()
            .unwrap();
        assert_eq!(av.hash, bv.hash);
        assert_ne!(av.location, bv.location);
        assert_eq!(av.hash, hash(n + 100));
    }
    let foreign = LocatedRoot::from_parts(a.parts().0, b.parts().1);
    assert_eq!(
        foreign.hash(),
        a.hash(),
        "locations are never root authority"
    );
    assert!(matches!(
        foreign.lookup(&a.hash(), &hash(1), |reference| first.read(reference)),
        Err(MerkleMapReadError::MissingNode(_))
    ));
}

#[test]
fn wrong_root_and_child_locations_fail_without_inventing_absence() {
    let left_key = Hash::prehashed([0; 32]);
    let right_key = Hash::prehashed([255; 32]);
    let mut map = MerkleMap::new();
    map.replace(left_key, None, Some(hash(1))).unwrap();
    map.replace(right_key, None, Some(hash(2))).unwrap();
    let mut store = Store::default();
    let root = export(&map, &mut store);
    let top = root.parts().1.unwrap();
    let LocatedNode::Branch {
        bit,
        prefix,
        left,
        right,
    } = store.nodes[&top.location]
    else {
        panic!("two-key branch");
    };
    for wrong in [u128::MAX, right.location] {
        let bad_root = LocatedRoot::from_parts(
            2,
            Some(MerkleMapNodeRef {
                hash: top.hash,
                location: wrong,
            }),
        );
        let error = bad_root
            .lookup(&root.hash(), &left_key, |reference| store.read(reference))
            .unwrap_err();
        assert_eq!(
            error,
            if wrong == u128::MAX {
                MerkleMapReadError::MissingNode(top.hash)
            } else {
                MerkleMapReadError::NodeHashMismatch(top.hash)
            }
        );
        store.nodes.insert(
            top.location,
            LocatedNode::Branch {
                bit,
                prefix,
                left: MerkleMapNodeRef {
                    hash: left.hash,
                    location: wrong,
                },
                right,
            },
        );
        let error = root
            .lookup(&root.hash(), &left_key, |reference| store.read(reference))
            .unwrap_err();
        assert_eq!(
            error,
            if wrong == u128::MAX {
                MerkleMapReadError::MissingNode(left.hash)
            } else {
                MerkleMapReadError::NodeHashMismatch(left.hash)
            }
        );
    }
}

#[test]
fn equal_child_hashes_are_invalid_even_at_different_physical_locations() {
    let leaf = LocatedNode::Leaf {
        key: hash(1),
        value: value(hash(2)),
    };
    let branch = LocatedNode::Branch {
        bit: 0,
        prefix: [0; 32],
        left: MerkleMapNodeRef {
            hash: leaf.hash(),
            location: 10,
        },
        right: MerkleMapNodeRef {
            hash: leaf.hash(),
            location: 11,
        },
    };
    let root = LocatedRoot::from_parts(
        2,
        Some(MerkleMapNodeRef {
            hash: branch.hash(),
            location: 12,
        }),
    );
    let mut reads = 0;
    assert_eq!(
        root.lookup(&root.hash(), &hash(1), |reference| {
            reads += 1;
            assert_eq!(reference.location, 12, "reject before following a child");
            Ok::<_, ()>(Some(branch))
        }),
        Err(MerkleMapReadError::InvalidPath)
    );
    assert_eq!(reads, 1);
}

#[test]
fn cold_export_keeps_its_original_version_at_every_value_and_node_failure() {
    let mut map = MerkleMap::new();
    for n in 0..16 {
        map.replace(hash(n), None, Some(hash(n + 100))).unwrap();
    }
    let original = map.root();
    for cut in 1..=16 {
        let mut store = Store::default();
        let mut values = 0;
        assert_eq!(
            map.export_nodes(&mut store, |_, hash| {
                values += 1;
                if values == cut {
                    Err("value unavailable")
                } else {
                    Ok(value(hash).location)
                }
            }),
            Err("value unavailable")
        );
        assert_eq!(values, cut);
        assert_eq!(map.root(), original);
        let retried = export(&map, &mut store);
        assert_eq!(retried.hash(), original);
        for n in 0..16 {
            assert_eq!(read(&retried, hash(n), &store), Some(hash(n + 100)));
        }
    }
    for cut in 1..=31 {
        let mut store = Store {
            fail_write: Some(cut),
            ..Store::default()
        };
        assert_eq!(
            map.export_nodes(&mut store, |_, hash| Ok(value(hash).location)),
            Err("admission refused")
        );
        assert_eq!(store.writes, cut);
        assert_eq!(map.root(), original);
        store.fail_write = None;
        assert_eq!(export(&map, &mut store).hash(), original);
    }
}
