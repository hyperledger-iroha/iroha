//! Streaming iterator regression coverage.

use std::{
    collections::{BTreeMap, HashMap},
    io::Cursor,
};

use norito::core as norito_core;
use norito::core::{DecodeFlagsGuard, header_flags};
use norito::{
    Compression, Error, NoritoSerialize, StreamMapIter, serialize_into,
    stream_btreemap_collect_from_reader, stream_hashmap_collect_from_reader, stream_seq_iter,
};

#[cfg(feature = "packed-struct")]
#[derive(Debug, Clone, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
struct PackedRow {
    id: u32,
    tag: u16,
}

fn overflow_varint() -> Vec<u8> {
    let mut bytes = vec![0x80; 9];
    bytes.push(0x02);
    bytes
}

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

#[test]
fn stream_seq_iter_compact_len_roundtrip() {
    let values: Vec<u32> = (0..256u32).map(|i| i.wrapping_mul(3) + 1).collect();
    let flags = header_flags::COMPACT_LEN | header_flags::COMPACT_SEQ_LEN;
    let payload = {
        let _guard = DecodeFlagsGuard::enter(flags);
        let mut payload = Vec::new();
        values.serialize(&mut payload).expect("serialize");
        payload
    };
    let bytes =
        norito_core::frame_bare_with_header_flags::<Vec<u32>>(&payload, flags).expect("frame");

    let mut iter = stream_seq_iter::<_, u32>(Cursor::new(bytes)).expect("iter");
    let collected: Vec<u32> = iter.by_ref().map(|v| v.expect("value")).collect();
    iter.finish().expect("finish");
    assert_eq!(collected, values);
    norito_core::reset_decode_state();
}

#[cfg(feature = "packed-struct")]
#[test]
fn stream_seq_iter_packed_struct_roundtrip() {
    let values = vec![
        PackedRow { id: 1, tag: 9 },
        PackedRow { id: 2, tag: 7 },
        PackedRow { id: 3, tag: 5 },
    ];
    let flags = header_flags::PACKED_STRUCT | header_flags::COMPACT_SEQ_LEN;
    let payload = {
        let _guard = DecodeFlagsGuard::enter(flags);
        let mut payload = Vec::new();
        values.serialize(&mut payload).expect("serialize");
        payload
    };
    let bytes = norito_core::frame_bare_with_header_flags::<Vec<PackedRow>>(&payload, flags)
        .expect("frame");

    let mut iter = stream_seq_iter::<_, PackedRow>(Cursor::new(bytes)).expect("iter");
    let collected: Vec<PackedRow> = iter.by_ref().map(|v| v.expect("value")).collect();
    iter.finish().expect("finish");
    assert_eq!(collected, values);
    norito_core::reset_decode_state();
}

#[test]
fn stream_map_compact_len_roundtrip() {
    let flags = header_flags::COMPACT_LEN | header_flags::COMPACT_SEQ_LEN;
    let mut hm: HashMap<String, u32> = HashMap::new();
    for i in 0..256u32 {
        hm.insert(format!("k-{}", i * 7), i.wrapping_mul(5) + 1);
    }
    let payload = {
        let _guard = DecodeFlagsGuard::enter(flags);
        let mut payload = Vec::new();
        hm.serialize(&mut payload).expect("serialize");
        payload
    };
    let bytes = norito_core::frame_bare_with_header_flags::<HashMap<String, u32>>(&payload, flags)
        .expect("frame");
    let iter_out: HashMap<String, u32> = StreamMapIter::new_hash(Cursor::new(bytes.clone()))
        .expect("iter")
        .map(|kv| kv.expect("entry"))
        .collect();
    assert_eq!(iter_out, hm);
    let collected =
        stream_hashmap_collect_from_reader::<_, String, u32>(Cursor::new(bytes)).expect("collect");
    assert_eq!(collected, hm);

    let mut bm: BTreeMap<u64, String> = BTreeMap::new();
    for i in 0..128u64 {
        bm.insert(i * 3 + 1, format!("v-{i}"));
    }
    let payload = {
        let _guard = DecodeFlagsGuard::enter(flags);
        let mut payload = Vec::new();
        bm.serialize(&mut payload).expect("serialize");
        payload
    };
    let bytes = norito_core::frame_bare_with_header_flags::<BTreeMap<u64, String>>(&payload, flags)
        .expect("frame");
    let collected =
        stream_btreemap_collect_from_reader::<_, u64, String>(Cursor::new(bytes)).expect("collect");
    assert_eq!(collected, bm);
    norito_core::reset_decode_state();
}

#[test]
fn stream_map_packed_varint_offsets_roundtrip() {
    let flags =
        header_flags::PACKED_SEQ | header_flags::VARINT_OFFSETS | header_flags::COMPACT_SEQ_LEN;
    let entries: Vec<(u8, u32)> = vec![(1, 10), (2, 20), (3, 30)];
    let mut key_payloads = Vec::new();
    let mut val_payloads = Vec::new();
    for (key, value) in entries.iter() {
        let mut key_buf = Vec::new();
        key.serialize(&mut key_buf).expect("serialize key");
        key_payloads.push(key_buf);
        let mut val_buf = Vec::new();
        value.serialize(&mut val_buf).expect("serialize value");
        val_payloads.push(val_buf);
    }
    let mut payload = Vec::new();
    norito_core::write_varint_len_to_vec(&mut payload, entries.len() as u64);
    for key_buf in &key_payloads {
        norito_core::write_varint_len_to_vec(&mut payload, key_buf.len() as u64);
    }
    for val_buf in &val_payloads {
        norito_core::write_varint_len_to_vec(&mut payload, val_buf.len() as u64);
    }
    for key_buf in &key_payloads {
        payload.extend_from_slice(key_buf);
    }
    for val_buf in &val_payloads {
        payload.extend_from_slice(val_buf);
    }
    let bytes = norito_core::frame_bare_with_header_flags::<HashMap<u8, u32>>(&payload, flags)
        .expect("frame");
    let iter_out: HashMap<u8, u32> = StreamMapIter::new_hash(Cursor::new(bytes.clone()))
        .expect("iter")
        .map(|kv| kv.expect("entry"))
        .collect();
    let expected: HashMap<u8, u32> = entries.into_iter().collect();
    assert_eq!(iter_out, expected);
    let collected =
        stream_hashmap_collect_from_reader::<_, u8, u32>(Cursor::new(bytes)).expect("collect");
    assert_eq!(collected, expected);
    norito_core::reset_decode_state();
}

#[test]
fn stream_seq_iter_rejects_overflowing_varint_len() {
    let flags = header_flags::COMPACT_SEQ_LEN;
    let payload = overflow_varint();
    let bytes =
        norito_core::frame_bare_with_header_flags::<Vec<u32>>(&payload, flags).expect("frame");
    let err = stream_seq_iter::<_, u32>(Cursor::new(bytes)).expect_err("iter");
    assert!(matches!(err, Error::LengthMismatch));
    norito_core::reset_decode_state();
}

#[test]
fn stream_map_iter_rejects_overflowing_entry_count_varint() {
    let flags = header_flags::COMPACT_SEQ_LEN;
    let payload = overflow_varint();
    let bytes = norito_core::frame_bare_with_header_flags::<HashMap<u8, u8>>(&payload, flags)
        .expect("frame");
    let err = StreamMapIter::<u8, u8>::new_hash(Cursor::new(bytes)).expect_err("iter");
    assert!(matches!(err, Error::LengthMismatch));
    norito_core::reset_decode_state();
}

#[test]
fn stream_map_iter_rejects_overflowing_key_len_varint() {
    let flags = header_flags::COMPACT_LEN;
    let mut payload = Vec::new();
    payload.extend_from_slice(&1u64.to_le_bytes());
    payload.extend_from_slice(&overflow_varint());
    let bytes = norito_core::frame_bare_with_header_flags::<HashMap<u8, u8>>(&payload, flags)
        .expect("frame");
    let mut iter = StreamMapIter::<u8, u8>::new_hash(Cursor::new(bytes)).expect("iter");
    let item = iter.next().expect("item");
    assert!(matches!(item, Err(Error::LengthMismatch)));
    norito_core::reset_decode_state();
}

#[test]
fn stream_map_collect_rejects_overflowing_key_len_varint() {
    let flags = header_flags::COMPACT_LEN;
    let mut payload = Vec::new();
    payload.extend_from_slice(&1u64.to_le_bytes());
    payload.extend_from_slice(&overflow_varint());
    let bytes = norito_core::frame_bare_with_header_flags::<HashMap<u8, u8>>(&payload, flags)
        .expect("frame");
    let err =
        stream_hashmap_collect_from_reader::<_, u8, u8>(Cursor::new(bytes)).expect_err("collect");
    assert!(matches!(err, Error::LengthMismatch));
    norito_core::reset_decode_state();
}
