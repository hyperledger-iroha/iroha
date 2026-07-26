use std::collections::{BTreeSet, HashMap, HashSet};

use norito::{
    Error,
    sequential::{
        deserialize_btreeset, deserialize_hashmap, deserialize_hashset, deserialize_vec,
        serialize_btreeset, serialize_hashmap, serialize_hashset, serialize_vec,
    },
};

#[test]
fn vec_sequential_roundtrip() -> Result<(), Error> {
    let values = vec![1u32, 2, 3, 4];
    let bytes = serialize_vec(&values)?;
    let decoded: Vec<u32> = deserialize_vec(&bytes)?;
    assert_eq!(decoded, values);
    Ok(())
}

#[test]
fn hashmap_sequential_roundtrip_is_deterministic() -> Result<(), Error> {
    let mut map_a = HashMap::new();
    map_a.insert("alice".to_owned(), 10u32);
    map_a.insert("bob".to_owned(), 20u32);
    map_a.insert("carol".to_owned(), 30u32);

    let mut map_b = HashMap::new();
    map_b.insert("carol".to_owned(), 30u32);
    map_b.insert("alice".to_owned(), 10u32);
    map_b.insert("bob".to_owned(), 20u32);

    let bytes_a = serialize_hashmap(&map_a)?;
    let bytes_b = serialize_hashmap(&map_b)?;
    assert_eq!(
        bytes_a, bytes_b,
        "sequential hashmap encoding must be canonical"
    );

    let decoded: HashMap<String, u32> = deserialize_hashmap(&bytes_a)?;
    assert_eq!(decoded.len(), map_a.len());
    for (key, value) in map_a {
        assert_eq!(decoded.get(&key).copied(), Some(value));
    }
    Ok(())
}

#[test]
fn hashset_sequential_roundtrip_is_deterministic() -> Result<(), Error> {
    let set_a: HashSet<_> = ["a", "b", "c"].into_iter().map(str::to_owned).collect();
    let set_b: HashSet<_> = ["c", "b", "a"].into_iter().map(str::to_owned).collect();

    let bytes_a = serialize_hashset(&set_a)?;
    let bytes_b = serialize_hashset(&set_b)?;
    assert_eq!(
        bytes_a, bytes_b,
        "sequential hashset encoding must be canonical"
    );

    let decoded: HashSet<String> = deserialize_hashset(&bytes_a)?;
    assert_eq!(decoded, set_a);
    Ok(())
}

#[test]
fn btreeset_sequential_roundtrip() -> Result<(), Error> {
    let set: BTreeSet<_> = [5u32, 1u32, 7u32].into_iter().collect();
    let bytes = serialize_btreeset(&set)?;
    let decoded: BTreeSet<u32> = deserialize_btreeset(&bytes)?;
    assert_eq!(decoded, set);
    Ok(())
}
