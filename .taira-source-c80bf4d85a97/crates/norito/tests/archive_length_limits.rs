#[cfg(target_pointer_width = "32")]
use std::panic::{AssertUnwindSafe, catch_unwind};
#[cfg(target_pointer_width = "32")]
use std::sync::Mutex;
use std::{
    collections::{BTreeMap, HashMap},
    io::Cursor,
};

use norito::{self, Error, core};

#[cfg(target_pointer_width = "32")]
fn with_limit<R>(limit: u64, f: impl FnOnce() -> R) -> R {
    static LIMIT_GUARD: Mutex<()> = Mutex::new(());
    let _lock = LIMIT_GUARD.lock().expect("limit mutex poisoned");
    let previous = core::max_archive_len();
    core::set_max_archive_len(limit);
    let outcome = catch_unwind(AssertUnwindSafe(f));
    core::set_max_archive_len(previous);
    match outcome {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn mutate_header_len(bytes: &[u8], new_len: u64) -> Vec<u8> {
    const LEN_OFFSET: usize = 4 + 1 + 1 + 16 + 1;
    let mut mutated = bytes.to_vec();
    mutated[LEN_OFFSET..LEN_OFFSET + 8].copy_from_slice(&new_len.to_le_bytes());
    mutated
}

#[test]
fn rejects_payloads_exceeding_configured_limit() {
    let payload = vec![1u8, 2, 3, 4, 5];
    let bytes = core::to_bytes(&payload).expect("serialize");
    let limit = core::max_archive_len();
    let inflated = mutate_header_len(&bytes, limit + 1);

    let err = match core::from_compressed_bytes::<Vec<u8>>(&inflated) {
        Ok(_) => panic!("from_compressed_bytes accepted oversize payload"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == limit + 1 && lim == limit
    ));

    let err = match norito::deserialize_stream::<_, Vec<u8>>(Cursor::new(&inflated)) {
        Ok(_) => panic!("deserialize_stream accepted oversize payload"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == limit + 1 && lim == limit
    ));
}

#[test]
fn streaming_sequences_reject_oversize_payloads() {
    let payload = vec![1u32, 2, 3];
    let bytes = norito::to_bytes(&payload).expect("serialize");
    let limit = core::max_archive_len();
    let inflated = mutate_header_len(&bytes, limit + 1);

    let err = norito::stream_vec_collect_from_reader::<_, u32>(Cursor::new(&inflated))
        .expect_err("stream vec must reject oversize payload");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == limit + 1 && lim == limit
    ));

    let err = match norito::stream_seq_iter::<_, u32>(Cursor::new(inflated)) {
        Ok(_) => panic!("stream seq iter accepted oversize payload"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == limit + 1 && lim == limit
    ));
}

#[test]
fn streaming_maps_reject_oversize_payloads() {
    let limit = core::max_archive_len();

    let mut map = HashMap::new();
    map.insert("k".to_string(), 7u32);
    let bytes = norito::to_bytes(&map).expect("serialize hashmap");
    let inflated = mutate_header_len(&bytes, limit + 1);

    let err = norito::stream_hashmap_collect_from_reader::<_, String, u32>(Cursor::new(&inflated))
        .expect_err("stream hashmap must reject oversize payload");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == limit + 1 && lim == limit
    ));

    let err = match norito::StreamMapIter::<String, u32>::new_hash(Cursor::new(inflated)) {
        Ok(_) => panic!("stream map iter accepted oversize payload"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == limit + 1 && lim == limit
    ));

    let mut btree = BTreeMap::new();
    btree.insert("k".to_string(), 9u32);
    let bytes = norito::to_bytes(&btree).expect("serialize btreemap");
    let inflated = mutate_header_len(&bytes, limit + 1);

    let err = norito::stream_btreemap_collect_from_reader::<_, String, u32>(Cursor::new(&inflated))
        .expect_err("stream btreemap must reject oversize payload");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == limit + 1 && lim == limit
    ));
}

#[test]
fn streaming_sequence_counts_respect_archive_limit() {
    let limit = core::max_archive_len();
    let oversized = limit + 1;
    let mut payload = Vec::new();
    payload.extend_from_slice(&oversized.to_le_bytes());

    let framed =
        core::frame_bare_with_header_flags::<Vec<u32>>(&payload, 0).expect("frame payload");
    let err = norito::stream_vec_collect_from_reader::<_, u32>(Cursor::new(&framed))
        .expect_err("stream vec must reject oversize count");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == oversized && lim == limit
    ));
}

#[test]
fn streaming_map_entry_lengths_respect_archive_limit() {
    let limit = core::max_archive_len();
    let oversized = limit + 1;
    let mut payload = Vec::new();
    payload.extend_from_slice(&1u64.to_le_bytes());
    payload.extend_from_slice(&oversized.to_le_bytes());
    payload.extend_from_slice(&0u64.to_le_bytes());

    let framed = core::frame_bare_with_header_flags::<HashMap<String, u32>>(&payload, 0)
        .expect("frame payload");
    let err = norito::stream_hashmap_collect_from_reader::<_, String, u32>(Cursor::new(&framed))
        .expect_err("stream hashmap must reject oversize key length");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == oversized && lim == limit
    ));

    let framed = core::frame_bare_with_header_flags::<BTreeMap<String, u32>>(&payload, 0)
        .expect("frame payload");
    let err = norito::stream_btreemap_collect_from_reader::<_, String, u32>(Cursor::new(&framed))
        .expect_err("stream btreemap must reject oversize key length");
    assert!(matches!(
        err,
        Error::ArchiveLengthExceeded {
            length,
            limit: lim
        } if length == oversized && lim == limit
    ));
}

#[cfg(target_pointer_width = "32")]
#[test]
fn rejects_lengths_overflowing_usize_on_32bit() {
    with_limit(u64::MAX, || {
        let payload = vec![0u8; 1];
        let bytes = core::to_bytes(&payload).expect("serialize");
        let pointer_cap = usize::MAX as u64;
        let oversized = mutate_header_len(&bytes, pointer_cap + 1);

        let err = match core::from_compressed_bytes::<Vec<u8>>(&oversized) {
            Ok(_) => panic!("from_compressed_bytes accepted overflow length"),
            Err(err) => err,
        };
        assert!(matches!(
            err,
            Error::ArchiveLengthExceeded { length, limit }
                if length == pointer_cap + 1 && limit == pointer_cap
        ));
    });
}
