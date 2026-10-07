use super::*;
use crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1 as digest;
use std::collections::BTreeMap;

#[derive(Default)]
struct Memory {
    objects: BTreeMap<[u8; 32], Vec<u8>>,
    reads: usize,
    fail_after: Option<usize>,
}
impl ObjectStore for Memory {
    fn read_object(&mut self, key: &[u8; 32], limit: usize) -> Result<Vec<u8>, Error> {
        self.reads += 1;
        let bytes = self
            .objects
            .get(key)
            .ok_or(Error::WitnessLost("test missing object"))?;
        if bytes.len() > limit {
            return Err(Error::WitnessLost("test bound"));
        }
        Ok(bytes.clone())
    }
    fn write_object(&mut self, bytes: &[u8], limit: usize) -> Result<[u8; 32], Error> {
        if let Some(left) = &mut self.fail_after {
            if *left == 0 {
                return Err(Error::Storage(std::io::Error::other("test unavailable")));
            }
            *left -= 1;
        }
        assert!(bytes.len() <= limit);
        let hash = digest(bytes);
        if let Some(held) = self.objects.get(&hash) {
            assert_eq!(held, bytes);
        }
        self.objects.insert(hash, bytes.to_vec());
        Ok(hash)
    }
}
fn field(number: u8) -> [u8; 32] {
    let mut x = [0; 32];
    x[0] = number;
    x
}

#[test]
fn persistent_map_matches_actual_native_intermediate_witnesses_after_restart_and_removal() {
    let mut store = Memory::default();
    let mut map = PersistentMapV1::default();
    let mut native = KagemushaWalletIndexedTreeV1::new();
    let mut retained = BTreeMap::new();
    for number in [19, 7, 25, 2, 31, 11, 3, 9] {
        let key = field(number);
        let value = field(number + 42);
        assert_eq!(
            map.non_membership(&mut store, &key).unwrap(),
            native.non_membership(&key).unwrap()
        );
        let before = map.root();
        let actual = map.insert(&mut store, key, value).unwrap();
        let expected = native.insert(key, value).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.verify(&before, &key, &value).unwrap(), native.root());
        assert_eq!(map.root(), native.root());
        retained.insert(key, value);
        // Restore exact immutable metadata and all real path objects, without scanning history.
        map = archive::decode(&archive::encode(&map).unwrap()).unwrap();
        for held in retained.keys() {
            store.reads = 0;
            assert_eq!(
                map.membership(&mut store, held).unwrap(),
                native.membership(held).unwrap()
            );
            assert!(
                store.reads <= 34 * 257,
                "bounded paths independent of history"
            );
        }
    }
    let original = map.clone();
    let original_native = native.clone();
    for number in [2, 25, 19, 3, 31, 7, 11, 9] {
        let key = field(number);
        let before = map.root();
        let actual = map.remove(&mut store, &key).unwrap();
        assert_eq!(actual, native.remove(&key).unwrap());
        assert_eq!(actual.verify(&before, &key).unwrap(), native.root());
        assert_eq!(map.root(), native.root());
        assert_eq!(
            map.non_membership(&mut store, &key).unwrap(),
            native.non_membership(&key).unwrap()
        );
    }
    assert_eq!(map.root(), kagemusha_wallet_empty_map_root_v1());
    for key in retained.keys() {
        assert_eq!(
            original.membership(&mut store, key).unwrap(),
            original_native.membership(key).unwrap()
        );
    }
    let new = map.insert(&mut store, field(2), field(88)).unwrap();
    assert_eq!(new, native.insert(field(2), field(88)).unwrap());
    assert_eq!(
        new.slot_opening.slot, 9,
        "removed physical slots never reused"
    );
}

#[test]
fn uncertain_insert_or_remove_preserves_the_selected_descriptor_and_its_exact_paths() {
    let mut store = Memory::default();
    let mut map = PersistentMapV1::default();
    map.insert(&mut store, field(3), field(4)).unwrap();
    let original = archive::encode(&map).unwrap();
    let member = map.membership(&mut store, &field(3)).unwrap();
    store.fail_after = Some(7);
    assert!(matches!(
        map.insert(&mut store, field(6), field(8)),
        Err(Error::Storage(_))
    ));
    assert_eq!(archive::encode(&map).unwrap(), original);
    store.fail_after = None;
    assert_eq!(map.membership(&mut store, &field(3)).unwrap(), member);
    store.fail_after = Some(7);
    assert!(matches!(
        map.remove(&mut store, &field(3)),
        Err(Error::Storage(_))
    ));
    assert_eq!(archive::encode(&map).unwrap(), original);
    store.fail_after = None;
    assert_eq!(map.membership(&mut store, &field(3)).unwrap(), member);
    assert!(map.non_membership(&mut store, &field(6)).is_ok());
}

#[test]
fn selected_object_loss_corruption_and_wrong_root_never_become_absence() {
    let mut store = Memory::default();
    let mut map = PersistentMapV1::default();
    map.insert(&mut store, field(3), field(4)).unwrap();
    let nodes = map.nodes.0;
    let original = store.objects.remove(&nodes).unwrap();
    assert!(matches!(
        map.non_membership(&mut store, &field(7)),
        Err(Error::WitnessLost(_))
    ));
    store.objects.insert(nodes, vec![0; original.len()]);
    assert!(matches!(
        map.membership(&mut store, &field(3)),
        Err(Error::WitnessLost(_))
    ));
    store.objects.insert(nodes, original);
    map.root = kagemusha_wallet_empty_map_root_v1();
    assert!(matches!(
        map.non_membership(&mut store, &field(7)),
        Err(Error::WitnessLost(_))
    ));
    assert!(map.insert(&mut store, field(8), field(9)).is_err());
}
