//! Regression tests for collections containing zero-sized elements.

use std::collections::{HashMap, VecDeque};

use norito::core::{decode_from_bytes, to_bytes};

#[test]
fn hashmap_unit_roundtrip() {
    let mut map: HashMap<(), ()> = HashMap::new();
    map.insert((), ());

    let bytes = to_bytes(&map).expect("serialize HashMap<(), ()>");
    let decoded: HashMap<(), ()> = decode_from_bytes(&bytes).expect("deserialize HashMap<(), ()>");

    assert_eq!(decoded, map);
    assert_eq!(decoded.len(), 1);
}

#[test]
fn vecdeque_unit_roundtrip() {
    let mut queue: VecDeque<()> = VecDeque::new();
    for _ in 0..4 {
        queue.push_back(());
    }

    let bytes = to_bytes(&queue).expect("serialize VecDeque<()> with unit elements");
    match decode_from_bytes::<VecDeque<()>>(&bytes) {
        Ok(_) => panic!("VecDeque<()> should be rejected under canonical encoding"),
        Err(err) => assert!(matches!(err, norito::Error::LengthMismatch)),
    }
}
