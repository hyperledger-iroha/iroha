// Batched reference-count merges must equal one sorted increment or decrement per chunk.
fn single_step_increment(entries: &mut Vec<ChunkRefcountEntry>, digest: [u8; 32]) {
    match entries.binary_search_by_key(&digest, |entry| entry.digest) {
        Ok(index) => entries[index].count += 1,
        Err(index) => entries.insert(index, ChunkRefcountEntry { digest, count: 1 }),
    }
}
fn single_step_decrement(entries: &mut Vec<ChunkRefcountEntry>, digest: [u8; 32]) {
    let index = entries
        .binary_search_by_key(&digest, |entry| entry.digest)
        .expect("decremented digest is present");
    if entries[index].count == 1 {
        entries.remove(index);
    } else {
        entries[index].count -= 1;
    }
}
fn refcount_test_digest(value: u8) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    digest[0] = value;
    digest[31] = value.wrapping_mul(7);
    digest
}
#[test]
fn batched_refcount_merges_match_single_step_updates() {
    let path = Path::new("refcount-merge-test-index");
    let mut state = 0x9E37_79B9_u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let mut merged = Vec::new();
    let mut expected = Vec::new();
    for round in 0..64 {
        // A small digest alphabet makes batches repeat digests and collide with existing rows.
        let length = next() % 24;
        let batch = (0..length)
            .map(|_| refcount_test_digest((next() % 40) as u8))
            .collect::<Vec<_>>();
        merged = refcounts_with_added(&merged, batch.iter().copied(), path).expect("add");
        for digest in &batch {
            single_step_increment(&mut expected, *digest);
        }
        assert_eq!(merged, expected, "add round {round}");
        let removal = &batch[..batch.len() / 2];
        merged = refcounts_with_removed(&merged, removal.iter().copied(), path).expect("remove");
        for digest in removal {
            single_step_decrement(&mut expected, *digest);
        }
        assert_eq!(merged, expected, "remove round {round}");
    }
    // Startup counts the digests of every manifest at once, in any order.
    let mut all_digests = expected
        .iter()
        .flat_map(|entry| std::iter::repeat_n(entry.digest, entry.count as usize))
        .collect::<Vec<_>>();
    all_digests.reverse();
    let startup = counted_refcounts(all_digests, path).expect("startup counts");
    assert_eq!(startup, expected);
    assert!(
        counted_refcounts(Vec::new(), path)
            .expect("empty")
            .is_empty()
    );
}
#[test]
fn batched_refcount_merges_refuse_overflow_and_missing_references() {
    let path = Path::new("refcount-refusal-test-index");
    let digest = refcount_test_digest(200);
    assert!(refcounts_with_removed(&[], [digest].into_iter(), path).is_err());
    let one = refcounts_with_added(&[], [digest].into_iter(), path).expect("add one");
    assert!(refcounts_with_removed(&one, [digest, digest].into_iter(), path).is_err());
    assert!(
        refcounts_with_removed(&one, [digest].into_iter(), path)
            .expect("remove last reference")
            .is_empty()
    );
    let saturated = [ChunkRefcountEntry {
        digest,
        count: u32::MAX,
    }];
    assert!(refcounts_with_added(&saturated, [digest].into_iter(), path).is_err());
}
