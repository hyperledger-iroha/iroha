//! Current/undo wire checks, including compound keys and deleted records.

use super::*;

fn fixture() -> Storage<(u64, u64), u64> {
    let store: Storage<_, _> = [((1, 0), 10), ((2, 0), 20)].into_iter().collect();
    let mut block = store.block();
    block.insert((1, 0), 11);
    block.remove((2, 0));
    block.insert((3, 0), 30);
    block.remove((4, 0));
    block.commit();
    store
}

fn envelope(store: &Storage<(u64, u64), u64>) -> SnapshotStorage {
    let mut encoded = String::new();
    serialize(store, &mut encoded);
    json::from_str(&encoded).unwrap()
}

#[test]
fn compound_snapshot_roundtrips_current_undo_and_staged_replacement() {
    let source = fixture();
    let restored = envelope(&source)
        .decode::<(u64, u64), u64>("fixture", |_, _| true)
        .unwrap();
    let mut expected = String::new();
    serialize(&source, &mut expected);
    let mut actual = String::new();
    serialize(&restored, &mut actual);
    assert_eq!(actual, expected);
    let mut replacement = restored.block_and_revert();
    assert_eq!(
        replacement
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        [((1, 0), 10), ((2, 0), 20)]
    );
    replacement.insert((2, 0), 22);
    let mut staged = String::new();
    serialize_block(&replacement, &mut staged);
    replacement.commit();
    actual.clear();
    serialize(&restored, &mut actual);
    assert_eq!(
        staged, actual,
        "staged serialization is the actual committed pair of maps"
    );
    let replacement_snapshot: SnapshotStorage = json::from_str(&staged).unwrap();
    let again = replacement_snapshot
        .decode::<(u64, u64), u64>("fixture", |_, _| true)
        .unwrap();
    assert_eq!(again.block_and_revert().get(&(2, 0)), Some(&20));
    expected.clear();
    serialize(&source, &mut expected);
    assert_ne!(
        actual, expected,
        "source snapshots remain untouched by replacement"
    );
}

#[test]
fn snapshot_rejects_duplicate_or_reordered_keys_in_both_maps() {
    for undo in [false, true] {
        for duplicate in [false, true] {
            let mut data = envelope(&fixture());
            if undo {
                if duplicate {
                    data.revert[1].key = data.revert[0].key.clone();
                } else {
                    data.revert.swap(0, 1);
                }
            } else if duplicate {
                data.blocks[1].key = data.blocks[0].key.clone();
            } else {
                data.blocks.swap(0, 1);
            }
            assert!(
                data.decode::<(u64, u64), u64>("fixture", |_, _| true)
                    .is_err()
            );
        }
    }
}

#[test]
fn snapshot_rejects_mismatched_records_in_both_maps_and_retains_absence() {
    let source = fixture();
    for reject in [11, 10] {
        assert!(
            envelope(&source)
                .decode::<(u64, u64), u64>("fixture", |_, value| *value != reject)
                .is_err()
        );
    }
    let restored = envelope(&source)
        .decode::<(u64, u64), u64>("fixture", |_, _| true)
        .unwrap();
    assert_eq!(restored.snapshot().revert_map().get(&(4, 0)), Some(&None));
    assert_eq!(
        restored.snapshot().revert_map().get(&(2, 0)),
        Some(&Some(20))
    );
}

#[test]
fn snapshot_requires_exact_norito_for_keys_values_and_preimages() {
    for location in 0..4 {
        let mut data = envelope(&fixture());
        let record = match location {
            0 => &mut data.blocks[0].key,
            1 => &mut data.blocks[0].value,
            2 => &mut data.revert[0].key,
            _ => data.revert[0].value.as_mut().unwrap(),
        };
        record.encoded_hex.push_str("00");
        assert!(
            data.decode::<(u64, u64), u64>("fixture", |_, _| true)
                .is_err()
        );
    }
}

#[test]
fn current_only_snapshot_shape_is_rejected() {
    for input in ["[]", "{\"blocks\":[]}", "{\"revert\":[]}"] {
        assert!(json::from_str::<SnapshotStorage>(input).is_err(), "{input}");
    }
}

#[test]
fn manifest_current_and_undo_require_exact_outer_identity_and_content_hash() {
    use crate::nexus::space_directory::SpaceDirectoryManifestRecord;
    use iroha_data_model::nexus::{AssetPermissionManifest, ManifestVersion};
    let uaid = UniversalAccountId::from_hash(Hash::new(b"snapshot manifest owner"));
    let manifest = AssetPermissionManifest {
        version: ManifestVersion::default(),
        uaid,
        dataspace: DataSpaceId::UNIVERSAL,
        issued_ms: 0,
        activation_epoch: 0,
        expiry_epoch: None,
        entries: Vec::new(),
    };
    for wrong_hash in [false, true] {
        let mut record = SpaceDirectoryManifestRecord::new(manifest.clone());
        let outer = if wrong_hash {
            record.manifest_hash = Hash::new(b"wrong manifest content");
            uaid
        } else {
            UniversalAccountId::from_hash(Hash::new(b"another snapshot manifest owner"))
        };
        let mut invalid = SpaceDirectoryManifestSet::default();
        invalid.upsert(record);
        for undo in [false, true] {
            let store = Storage::from_snapshot_parts(
                if undo {
                    BTreeMap::new()
                } else {
                    BTreeMap::from([(outer, invalid.clone())])
                },
                if undo {
                    BTreeMap::from([(outer, Some(invalid.clone()))])
                } else {
                    BTreeMap::new()
                },
            );
            let mut bytes = String::new();
            serialize(&store, &mut bytes);
            let snapshot: SnapshotStorage = json::from_str(&bytes).unwrap();
            assert!(
                snapshot
                    .decode("space_directory_manifests", manifest_set_matches_key)
                    .is_err(),
                "both identity and digest are validated even for deleted predecessor records"
            );
        }
    }
    let mut valid = SpaceDirectoryManifestSet::default();
    valid.upsert(SpaceDirectoryManifestRecord::new(manifest));
    assert!(manifest_set_matches_key(&uaid, &valid));
}
