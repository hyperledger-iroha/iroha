#![allow(clippy::manual_div_ceil)]
use std::collections::{BTreeMap, HashMap};

use norito::{
    Compression,
    core::{NoritoDeserialize, NoritoSerialize},
    deserialize_stream, serialize_into, stream_btreeset_collect_from_reader,
    stream_hashset_collect_from_reader, stream_linkedlist_collect_from_reader,
    stream_vec_collect_from_reader, stream_vec_fold_from_reader,
    stream_vecdeque_collect_from_reader,
};

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct BigData(Vec<u8>);

#[repr(align(64))]
#[derive(
    Clone, Copy, Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema,
)]
struct AlignedWord(u128);

#[test]
fn stream_large_payload_no_compression() {
    let data = BigData(vec![42; 5 * 1024 * 1024]);
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::None).unwrap();
    let decoded: BigData = deserialize_stream(buf.as_slice()).unwrap();
    assert_eq!(data, decoded);
}

#[test]
fn stream_large_payload_zstd() {
    let data = BigData(vec![7; 5 * 1024 * 1024]);
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::Zstd).unwrap();
    let decoded: BigData = deserialize_stream(buf.as_slice()).unwrap();
    assert_eq!(data, decoded);
}

#[test]
fn stream_vec_fold_uncompressed() {
    let data: Vec<u32> = (0..10_000u32).map(|i| i.wrapping_mul(3) + 1).collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::None).unwrap();
    // Fold to sum via streaming
    let sum: u64 =
        stream_vec_fold_from_reader::<_, u32, u64, _>(buf.as_slice(), 0, |acc, v| acc + v as u64)
            .unwrap();
    let expect: u64 = data.iter().map(|&v| v as u64).sum();
    assert_eq!(sum, expect);
}

#[test]
fn stream_vec_collect_zstd() {
    let data: Vec<String> = (0..2048).map(|i| format!("str-{}-{}", i, i * 17)).collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::Zstd).unwrap();
    let out: Vec<String> = stream_vec_collect_from_reader(buf.as_slice()).unwrap();
    assert_eq!(out, data);
}

#[test]
fn stream_vec_fold_handles_large_elements() {
    let payload: Vec<Vec<u8>> = (0..32)
        .map(|i| vec![i as u8; 64 * 1024 + (i as usize)])
        .collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &payload, Compression::None).expect("serialize large elements");

    let total_len: usize =
        stream_vec_fold_from_reader::<_, Vec<u8>, usize, _>(buf.as_slice(), 0, |acc, chunk| {
            acc + chunk.len()
        })
        .expect("stream fold large elements");

    let expected: usize = payload.iter().map(|chunk| chunk.len()).sum();
    assert_eq!(total_len, expected);
}

#[test]
fn stream_vec_collect_handles_alignment_padding() {
    let data: Vec<AlignedWord> = (0..128)
        .map(|i| AlignedWord((i as u128) << 32 | 0xA5A5))
        .collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::None).unwrap();

    let align = std::mem::align_of::<norito::core::Archived<Vec<AlignedWord>>>();
    let expected_padding = if align <= 1 {
        0
    } else {
        let remainder = norito::core::Header::SIZE % align;
        if remainder == 0 { 0 } else { align - remainder }
    };
    let payload_len = {
        let mut bytes = [0u8; 8];
        bytes
            .copy_from_slice(&buf[norito::core::Header::SIZE - 17..norito::core::Header::SIZE - 9]);
        u64::from_le_bytes(bytes) as usize
    };
    let padding = buf.len() - norito::core::Header::SIZE - payload_len;
    assert_eq!(padding, expected_padding);

    let collected: Vec<AlignedWord> = stream_vec_collect_from_reader(buf.as_slice()).unwrap();
    assert_eq!(collected, data);

    let folded: u128 = stream_vec_fold_from_reader::<_, AlignedWord, u128, _>(
        buf.as_slice(),
        0,
        |acc, AlignedWord(v)| acc.wrapping_add(v),
    )
    .unwrap();
    let expected: u128 = data.iter().map(|AlignedWord(v)| *v).sum();
    assert_eq!(folded, expected);
}

#[test]
fn stream_vec_collect_rejects_nonzero_padding() {
    let data: Vec<AlignedWord> = (0..8)
        .map(|i| AlignedWord((i as u128) << 32 | 0xBEEF))
        .collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::None).unwrap();
    let payload_len = {
        let mut bytes = [0u8; 8];
        bytes
            .copy_from_slice(&buf[norito::core::Header::SIZE - 17..norito::core::Header::SIZE - 9]);
        u64::from_le_bytes(bytes) as usize
    };
    let padding = buf.len() - norito::core::Header::SIZE - payload_len;
    if padding == 0 {
        buf[norito::core::Header::SIZE] ^= 0xFF;
    } else {
        buf[norito::core::Header::SIZE] = 0xFF;
    }

    let out: Result<Vec<AlignedWord>, _> = stream_vec_collect_from_reader(buf.as_slice());
    assert!(out.is_err());
}

#[test]
fn deserialize_stream_rejects_nonzero_padding() {
    let data = AlignedWord(0xCAFE_BABE_DEAD_BEEF);
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::None).unwrap();
    let payload_len = {
        let mut bytes = [0u8; 8];
        bytes
            .copy_from_slice(&buf[norito::core::Header::SIZE - 17..norito::core::Header::SIZE - 9]);
        u64::from_le_bytes(bytes) as usize
    };
    let padding = buf.len() - norito::core::Header::SIZE - payload_len;
    assert!(padding > 0);
    buf[norito::core::Header::SIZE] = 1;

    let decoded: Result<AlignedWord, _> = deserialize_stream(buf.as_slice());
    assert!(decoded.is_err());
}

#[test]
fn stream_vec_iter_skips_alignment_padding() {
    use std::io::Cursor;

    let data: Vec<AlignedWord> = (0..32)
        .map(|i| AlignedWord((i as u128) << 48 | 0x55AA))
        .collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::None).unwrap();
    let align = std::mem::align_of::<norito::core::Archived<Vec<AlignedWord>>>();
    let expected_padding = if align <= 1 {
        0
    } else {
        let remainder = norito::core::Header::SIZE % align;
        if remainder == 0 { 0 } else { align - remainder }
    };
    let payload_len = {
        let mut bytes = [0u8; 8];
        bytes
            .copy_from_slice(&buf[norito::core::Header::SIZE - 17..norito::core::Header::SIZE - 9]);
        u64::from_le_bytes(bytes) as usize
    };
    let padding = buf.len() - norito::core::Header::SIZE - payload_len;
    assert_eq!(padding, expected_padding);

    let mut iter =
        norito::stream_seq_iter::<_, AlignedWord>(Cursor::new(buf.clone())).expect("stream iter");
    let mut decoded = Vec::new();
    for item in iter.by_ref() {
        decoded.push(item.expect("stream element"));
    }
    iter.finish().expect("finish iterator");
    assert_eq!(decoded, data);
}

#[test]
fn stream_vecdeque_collect_uncompressed() {
    use std::collections::VecDeque;
    let data: VecDeque<u64> = (0..4096u64).map(|i| i * 7).collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::None).unwrap();
    let flags = buf[norito::core::Header::SIZE - 1];
    assert_eq!(
        flags,
        norito::core::default_encode_flags(),
        "stream header advertises unexpected flags: {flags:#x}"
    );
    let out = stream_vecdeque_collect_from_reader(buf.as_slice()).unwrap();
    assert_eq!(out, data);
}

#[test]
fn stream_linkedlist_collect_zstd() {
    use std::collections::LinkedList;
    let mut data: LinkedList<i32> = LinkedList::new();
    for i in 0..2048 {
        data.push_back(i - 1024);
    }
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::Zstd).unwrap();
    let out: LinkedList<i32> = stream_linkedlist_collect_from_reader(buf.as_slice()).unwrap();
    assert!(out.iter().eq(data.iter()));
}

#[test]
fn stream_sets_collect_uncompressed() {
    use std::collections::{BTreeSet, HashSet};
    let mut hs: HashSet<String> = HashSet::new();
    for i in 0..1000 {
        hs.insert(format!("k-{}", i * 3));
    }
    let mut buf = Vec::new();
    serialize_into(&mut buf, &hs, Compression::None).unwrap();
    let out_hs = stream_hashset_collect_from_reader::<_, String>(buf.as_slice()).unwrap();
    assert_eq!(out_hs, hs);

    let mut bs: BTreeSet<u32> = BTreeSet::new();
    for i in 0..4096 {
        bs.insert(i * 5);
    }
    let mut buf2 = Vec::new();
    serialize_into(&mut buf2, &bs, Compression::None).unwrap();
    let out_bs = stream_btreeset_collect_from_reader::<_, u32>(buf2.as_slice()).unwrap();
    assert_eq!(out_bs, bs);
}

#[test]
fn stream_maps_collect() {
    // HashMap uncompressed
    let mut hm: HashMap<String, u32> = HashMap::new();
    for i in 0..512u32 {
        hm.insert(format!("k-{}", i * 7), i * i + 1);
    }
    let mut buf = Vec::new();
    serialize_into(&mut buf, &hm, Compression::None).unwrap();
    let (len, compression) = {
        let mut bytes = [0u8; 8];
        bytes
            .copy_from_slice(&buf[norito::core::Header::SIZE - 17..norito::core::Header::SIZE - 9]);
        let comp = buf[norito::core::Header::SIZE - 18];
        (u64::from_le_bytes(bytes), comp)
    };
    println!(
        "hashmap hdr_len={} payload={} flags={} compression={}",
        len,
        buf.len() - norito::core::Header::SIZE,
        buf[norito::core::Header::SIZE - 1],
        compression
    );
    let out_hm =
        norito::stream_hashmap_collect_from_reader::<_, String, u32>(buf.as_slice()).unwrap();
    assert_eq!(hm, out_hm);

    // BTreeMap compressed
    let mut bm: BTreeMap<u64, String> = BTreeMap::new();
    for i in 0..1024u64 {
        bm.insert(i * 3 + 1, format!("v-{}", i * 11));
    }
    let mut buf2 = Vec::new();
    serialize_into(&mut buf2, &bm, Compression::Zstd).unwrap();
    let out_bm =
        norito::stream_btreemap_collect_from_reader::<_, u64, String>(buf2.as_slice()).unwrap();
    assert_eq!(bm, out_bm);
}

#[test]
fn stream_vec_canonical_header_small_payload() {
    let payload: Vec<Vec<u8>> = (0..64)
        .map(|i| vec![i as u8; (i as usize % 8) + 5])
        .collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &payload, Compression::None).unwrap();
    let flags = buf[norito::core::Header::SIZE - 1];
    assert_eq!(
        flags,
        norito::core::default_encode_flags(),
        "stream header advertises unexpected flags: {flags:#x}"
    );
    let streamed: Vec<Vec<u8>> = stream_vec_collect_from_reader(buf.as_slice()).unwrap();
    assert_eq!(streamed, payload);
}

#[test]
fn stream_vec_canonical_header_large_payload() {
    let payload: Vec<Vec<u8>> = (0..16)
        .map(|i| vec![i as u8; 8192 + (i as usize)])
        .collect();
    let mut buf = Vec::new();
    serialize_into(&mut buf, &payload, Compression::None).unwrap();
    let flags = buf[norito::core::Header::SIZE - 1];
    assert_eq!(
        flags,
        norito::core::default_encode_flags(),
        "stream header advertises unexpected flags: {flags:#x}"
    );
    let streamed: Vec<Vec<u8>> = stream_vec_collect_from_reader(buf.as_slice()).unwrap();
    assert_eq!(streamed, payload);
}
