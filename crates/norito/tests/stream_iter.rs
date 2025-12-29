use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use norito::{Compression, StreamMapIter, serialize_into, stream_seq_iter};

#[test]
fn stream_seq_iter_half_then_finish() {
    // Build a reasonably large Vec<u32>
    let v: Vec<u32> = (0..10_000u32).map(|i| i.wrapping_mul(7) + 3).collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &v, Compression::None).unwrap();

    // Iterate half, then finish()
    let mut it = stream_seq_iter::<_, u32>(Cursor::new(buf.clone())).unwrap();
    let mut sum = 0u64;
    for (i, x) in it.by_ref().enumerate() {
        let x = x.unwrap();
        sum = sum.wrapping_add(x as u64);
        if i >= v.len() / 2 {
            break;
        }
    }
    // Ensure finish() verifies integrity
    it.finish().unwrap();
    assert!(sum > 0);
}

#[test]
fn stream_map_iter_partial_then_finish() {
    // HashMap<String,u32>
    let mut hm: HashMap<String, u32> = HashMap::new();
    for i in 0..3000u32 {
        hm.insert(format!("k-{}", i * 11), i * 5 + 1);
    }
    let mut buf = Vec::new();
    serialize_into(&mut buf, &hm, Compression::None).unwrap();
    let mut it = StreamMapIter::<String, u32>::new_hash(Cursor::new(buf.clone())).unwrap();
    let mut cnt = 0usize;
    for kv in it.by_ref() {
        let (_k, _v) = kv.unwrap();
        cnt += 1;
        if cnt >= 1000 {
            break;
        }
    }
    it.finish().unwrap();
    assert!(cnt == 1000);

    // BTreeMap<u64,String> compressed streaming, consume fully via finish()
    let mut bm: BTreeMap<u64, String> = BTreeMap::new();
    for i in 0..5000u64 {
        bm.insert(i * 3 + 1, format!("v-{i}"));
    }
    let mut z = Vec::new();
    serialize_into(&mut z, &bm, Compression::Zstd).unwrap();
    let it2 = StreamMapIter::<u64, String>::new_btree(Cursor::new(z.clone())).unwrap();
    // Don't consume any entries; just finish and verify CRC
    it2.finish().unwrap();
}
