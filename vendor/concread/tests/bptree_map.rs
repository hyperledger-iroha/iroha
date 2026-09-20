use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use concread::bptree::BptreeMap;

proptest::proptest! {
    #[test]
    fn bptree_range_iter_consistent(values: BTreeSet<u8>, left in 0..u8::MAX - 1, len in 1..u8::MAX, bounds: (Bound<()>, Bound<()>)) {
        let range = (bounds.0.map(|()| left), bounds.1.map(|()| left.saturating_add(len)));
        let btree_map = BTreeMap::from_iter(values.iter().cloned().map(|v| (v, ())));
        let bptree_map = BptreeMap::from_iter(values.iter().cloned().map(|v| (v, ())));
        let bptree_map_read_tx = bptree_map.read();

        let btree_iter = btree_map.range(range);
        let bptree_iter = bptree_map_read_tx.range(range);

        assert!(
            btree_iter.eq(bptree_iter)
        )
    }

    #[test]
    fn bptree_get_consistent(values: BTreeSet<u8>, key: u8) {
        let btree_map = BTreeMap::from_iter(values.iter().cloned().map(|v| (v, v)));
        let bptree_map = BptreeMap::from_iter(values.iter().cloned().map(|v| (v, v)));
        let bptree_map_read_tx = bptree_map.read();

        let btree_value = btree_map.get(&key);
        let bptree_value = bptree_map_read_tx.get(&key);

        assert_eq!(btree_value, bptree_value);
    }

    #[test]
    fn bptree_remove_consistent(values in proptest::collection::btree_set(proptest::arbitrary::any::<u8>(), 1..256), indices: Vec<proptest::sample::Index> ) {
        let mut btree_map = BTreeMap::from_iter(values.iter().cloned().map(|v| (v.to_string(), v.to_string())));
        let bptree_map = BptreeMap::from_iter(values.iter().cloned().map(|v| (v.to_string(), v.to_string())));
        let mut bptree_map_write_tx = bptree_map.write();

        for index in indices {
            let index = index.index(values.len());
            let key = values.iter().nth(index).unwrap().to_string();

            assert_eq!(
                btree_map.remove(&key),
                bptree_map_write_tx.remove(&key)
            );

            let btree_value = btree_map.get(&key);
            assert_eq!(btree_value, None);
            let bptree_value = bptree_map_write_tx.get(&key);
            assert_eq!(bptree_value, None);

            assert!(
                btree_map.iter().eq(bptree_map_write_tx.iter())
            );
        }
    }
}

#[test]
fn bptree_remove_1() {
    let values = [
        4u8, 9, 12, 27, 34, 40, 59, 81, 89, 100, 142, 183, 189, 196, 218, 241,
    ];

    let to_remove = [9u8, 27, 40, 4].map(|v| v.to_string());

    let bptree_map = BptreeMap::from_iter(
        values
            .iter()
            .cloned()
            .map(|v| (v.to_string(), v.to_string())),
    );
    let mut bptree_map_write_tx = bptree_map.write();

    for key in to_remove {
        assert!(bptree_map_write_tx.remove(&key).is_some());
    }
}
