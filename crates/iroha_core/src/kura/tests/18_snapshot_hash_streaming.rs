mod snapshot_hash_streaming {
    //! Adversarial controls for the canonical snapshot hash-journal streaming owner.

    use super::*;
    use std::io::SeekFrom;

    fn hashes(count: usize) -> Vec<HashOf<BlockHeader>> {
        (0..count)
            .map(|index| {
                HashOf::from_untyped_unchecked(Hash::new(
                    u64::try_from(index).unwrap().to_le_bytes(),
                ))
            })
            .collect()
    }

    fn fixture(count: usize) -> (TempDir, BlockStore, Vec<HashOf<BlockHeader>>) {
        let directory = TempDir::new().expect("temporary hash journal");
        let mut store =
            BlockStore::with_fsync(directory.path(), FsyncMode::Batched, FSYNC_INTERVAL);
        store
            .create_files_if_they_do_not_exist()
            .expect("create the original empty canonical journals");
        let hashes = hashes(count);
        store
            .overwrite_block_hashes(&hashes)
            .expect("write canonical marked hash bytes");
        for index in 0..count {
            store
                .write_block_index(u64::try_from(index).unwrap(), EVICTED_BLOCK_START, 0)
                .expect("write a hash-only placeholder index");
        }
        (directory, store, hashes)
    }

    fn concatenation_digest(hashes: &[HashOf<BlockHeader>]) -> Hash {
        // Independent oracle: do not call either production streaming digest.
        let mut bytes = VERIFIED_SNAPSHOT_TAIL_DIGEST_DOMAIN.to_vec();
        bytes.extend_from_slice(&u64::try_from(hashes.len()).unwrap().to_le_bytes());
        for hash in hashes {
            bytes.extend_from_slice(hash.as_ref());
        }
        Hash::new(bytes)
    }

    fn journals(directory: &Path) -> Vec<Vec<u8>> {
        [
            DATA_FILE_NAME,
            INDEX_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ]
        .into_iter()
        .map(|name| std::fs::read(directory.join(name)).expect("read original journal bytes"))
        .collect()
    }

    fn marker_bytes(directory: &Path) -> Vec<u8> {
        std::fs::read(directory.join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME))
            .expect("read exact snapshot-tail marker bytes")
    }

    fn write_marker(store: &BlockStore, hashes: &[HashOf<BlockHeader>]) {
        store
            .write_verified_snapshot_tail_marker(
                0,
                hashes,
                Some(Hash::new(b"streaming-test-consistency-lineage")),
            )
            .expect("write consistency marker without minting snapshot authority");
    }

    fn set_hash_cursor(store: &mut BlockStore, position: u64) {
        store
            .ensure_hashes_file()
            .unwrap()
            .try_io(|file| file.seek(SeekFrom::Start(position)))
            .unwrap();
    }

    fn hash_cursor(store: &mut BlockStore) -> u64 {
        store
            .ensure_hashes_file()
            .unwrap()
            .try_io(|file| file.stream_position())
            .unwrap()
    }

    fn assert_range_error(error: Error, start: u64, count: usize) {
        assert!(
            matches!(error, Error::OutOfBoundsBlockRead {
                start_block_height,
                block_count,
            } if start_block_height == start && block_count == count),
            "a rejected hash range must retain its exact requested start and count"
        );
    }

    #[test]
    fn memory_and_disk_digests_match_exact_concatenation_across_chunk_boundaries() {
        // The production disk scratch contains 128 hashes. Exercise both sides
        // of each full chunk and more than two complete chunks.
        for count in [0, 1, 127, 128, 129, 256, 257] {
            let (directory, mut store, hashes) = fixture(count);
            let before = journals(directory.path());
            let expected = concatenation_digest(&hashes);
            assert_eq!(
                verified_snapshot_hash_journal_digest(&hashes).unwrap(),
                expected,
                "memory digest at height {count}",
            );
            assert_eq!(
                store
                    .verified_snapshot_hash_journal_digest_from_store(
                        u64::try_from(count).unwrap(),
                    )
                    .unwrap(),
                expected,
                "disk digest at height {count}",
            );
            assert_eq!(journals(directory.path()), before);
        }
    }

    #[test]
    fn disk_digest_reads_only_the_exact_prefix_and_ignores_corrupt_suffix() {
        let (directory, mut store, hashes) = fixture(258);
        let prefix = 129_usize;
        let path = directory.path().join(HASHES_FILE_NAME);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[prefix * Hash::LENGTH + Hash::LENGTH - 1] &= !1;
        std::fs::write(&path, &bytes).unwrap();
        let expected = concatenation_digest(&hashes[..prefix]);

        assert_eq!(
            store
                .verified_snapshot_hash_journal_digest_from_store(prefix as u64)
                .unwrap(),
            expected,
        );
        assert_eq!(hash_cursor(&mut store), prefix as u64 * SIZE_OF_BLOCK_HASH);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(matches!(
            store.verified_snapshot_hash_journal_digest_from_store((prefix + 1) as u64),
            Err(Error::IO(error, actual))
                if error.kind() == ErrorKind::InvalidData && actual == path
        ));
    }

    #[test]
    fn checked_range_preserves_empty_and_exact_end_semantics() {
        assert_eq!(checked_block_hash_read_range(0, 0, 0).unwrap(), (0, 0));
        assert_eq!(checked_block_hash_read_range(2, 0, 64).unwrap(), (64, 0));
        assert_eq!(checked_block_hash_read_range(1, 2, 96).unwrap(), (32, 64));
        assert_range_error(checked_block_hash_read_range(2, 0, 63).unwrap_err(), 2, 0);
        assert_range_error(checked_block_hash_read_range(1, 2, 95).unwrap_err(), 1, 2);
    }

    #[test]
    fn checked_range_rejects_offset_length_and_end_overflow() {
        let offset_overflow = u64::MAX / SIZE_OF_BLOCK_HASH + 1;
        assert_range_error(
            checked_block_hash_read_range(offset_overflow, 1, u64::MAX).unwrap_err(),
            offset_overflow,
            1,
        );
        let end_overflow = u64::MAX / SIZE_OF_BLOCK_HASH;
        assert_range_error(
            checked_block_hash_read_range(end_overflow, 1, u64::MAX).unwrap_err(),
            end_overflow,
            1,
        );
        // On 32-bit platforms no usize count can overflow a u64 byte length.
        if let Ok(count) = usize::try_from(offset_overflow) {
            assert_range_error(
                checked_block_hash_read_range(0, count, u64::MAX).unwrap_err(),
                0,
                count,
            );
        }
    }

    #[test]
    fn vector_reader_rejects_overflow_before_seek_or_returning_wrapped_hash() {
        let (directory, mut store, _) = fixture(2);
        let before = journals(directory.path());
        let mut cases = vec![
            (u64::MAX / SIZE_OF_BLOCK_HASH + 1, 1),
            (u64::MAX / SIZE_OF_BLOCK_HASH, 1),
            (0, 3),
        ];
        if let Ok(count) = usize::try_from(u64::MAX / SIZE_OF_BLOCK_HASH + 1) {
            cases.push((0, count));
        }
        for (start, count) in cases {
            set_hash_cursor(&mut store, 7);
            assert_range_error(
                store.read_block_hashes(start, count).unwrap_err(),
                start,
                count,
            );
            assert_eq!(
                hash_cursor(&mut store),
                7,
                "range refusal must precede seek"
            );
            assert_eq!(journals(directory.path()), before);
        }
    }

    #[test]
    fn disk_digest_rejects_oversized_height_before_seek() {
        let (directory, mut store, _) = fixture(2);
        let before = journals(directory.path());
        set_hash_cursor(&mut store, 7);
        let height = u64::MAX / SIZE_OF_BLOCK_HASH + 1;
        let error = store
            .verified_snapshot_hash_journal_digest_from_store(height)
            .unwrap_err();
        if let Ok(count) = usize::try_from(height) {
            assert_range_error(error, 0, count);
        } else {
            assert!(matches!(error, Error::IntConversion(_)));
        }
        assert_eq!(hash_cursor(&mut store), 7);
        assert_eq!(journals(directory.path()), before);
    }

    #[test]
    fn disk_digest_rejects_unmarked_entries_in_first_and_later_chunks() {
        for corrupt_index in [0, 127, 128, 256] {
            let (directory, mut store, _) = fixture(257);
            let path = directory.path().join(HASHES_FILE_NAME);
            let mut bytes = std::fs::read(&path).unwrap();
            bytes[corrupt_index * Hash::LENGTH + Hash::LENGTH - 1] &= !1;
            std::fs::write(&path, &bytes).unwrap();
            assert!(matches!(
                store.verified_snapshot_hash_journal_digest_from_store(257),
                Err(Error::IO(error, actual))
                    if error.kind() == ErrorKind::InvalidData && actual == path
            ));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn truncated_journal_is_rejected_before_seek_without_normalizing_evidence() {
        for retained_bytes in [0, 31, 32, 63] {
            let (directory, mut store, _) = fixture(2);
            let path = directory.path().join(HASHES_FILE_NAME);
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(retained_bytes)
                .unwrap();
            let before = journals(directory.path());
            set_hash_cursor(&mut store, 7);
            assert_range_error(
                store
                    .verified_snapshot_hash_journal_digest_from_store(2)
                    .unwrap_err(),
                0,
                2,
            );
            assert_eq!(hash_cursor(&mut store), 7);
            assert_eq!(journals(directory.path()), before);
        }
    }

    #[test]
    fn missing_journal_is_not_recreated_by_digest_read() {
        let (directory, mut store, _) = fixture(1);
        let path = directory.path().join(HASHES_FILE_NAME);
        store.drop_cached_handles();
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(
            store.verified_snapshot_hash_journal_digest_from_store(1),
            Err(Error::IO(error, actual))
                if error.kind() == ErrorKind::NotFound && actual == path
        ));
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn disk_digest_retains_the_original_open_hash_journal_owner() {
        let (directory, mut store, hashes) = fixture(129);
        set_hash_cursor(&mut store, 7);
        let original_path = directory.path().join(HASHES_FILE_NAME);
        let retained_path = directory.path().join("retained-original-hashes");
        std::fs::rename(&original_path, &retained_path).unwrap();
        let replacement = vec![0_u8; hashes.len() * Hash::LENGTH];
        std::fs::write(&original_path, &replacement).unwrap();
        let retained = std::fs::read(&retained_path).unwrap();

        assert_eq!(
            store
                .verified_snapshot_hash_journal_digest_from_store(129)
                .unwrap(),
            concatenation_digest(&hashes),
        );
        assert_eq!(std::fs::read(&original_path).unwrap(), replacement);
        assert_eq!(std::fs::read(&retained_path).unwrap(), retained);
    }

    #[test]
    fn both_marker_consumers_accept_exact_prefix_without_touching_evidence() {
        let (directory, mut store, hashes) = fixture(257);
        write_marker(&store, &hashes[..129]);
        let before = journals(directory.path());
        let marker_before = marker_bytes(directory.path());
        for repair in [false, true] {
            let marker = if repair {
                store.validated_verified_snapshot_tail(257, 257)
            } else {
                store.validated_verified_snapshot_tail_read_only(257, 257)
            }
            .unwrap()
            .expect("valid prefix marker");
            assert_eq!(marker.snapshot_height, 129);
            assert_eq!(
                marker.hash_journal_digest,
                concatenation_digest(&hashes[..129])
            );
            assert_eq!(marker_bytes(directory.path()), marker_before);
            assert_eq!(journals(directory.path()), before);
        }
    }

    #[test]
    fn read_only_digest_mismatch_preserves_marker_and_repair_removes_only_marker() {
        let (directory, mut store, hashes) = fixture(129);
        write_marker(&store, &hashes);
        store.write_block_hash(128, hashes[0]).unwrap();
        let before = journals(directory.path());
        let marker_before = marker_bytes(directory.path());
        let marker_path = directory.path().join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME);

        assert!(matches!(
            store.validated_verified_snapshot_tail_read_only(129, 129),
            Err(Error::InvalidSnapshotBootstrapMarker { path, reason })
                if path == marker_path && reason.contains("hash-journal digest")
        ));
        assert_eq!(marker_bytes(directory.path()), marker_before);
        assert_eq!(journals(directory.path()), before);
        assert!(
            store
                .validated_verified_snapshot_tail(129, 129)
                .unwrap()
                .is_none()
        );
        assert!(!marker_path.exists());
        assert_eq!(journals(directory.path()), before);
    }

    #[test]
    fn both_marker_consumers_preserve_marker_on_unmarked_hash_io_error() {
        let (directory, mut store, hashes) = fixture(129);
        write_marker(&store, &hashes);
        let path = directory.path().join(HASHES_FILE_NAME);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[128 * Hash::LENGTH + Hash::LENGTH - 1] &= !1;
        std::fs::write(&path, bytes).unwrap();
        let before = journals(directory.path());
        let marker_before = marker_bytes(directory.path());
        for repair in [false, true] {
            let result = if repair {
                store.validated_verified_snapshot_tail(129, 129)
            } else {
                store.validated_verified_snapshot_tail_read_only(129, 129)
            };
            assert!(matches!(result, Err(Error::IO(error, actual))
                if error.kind() == ErrorKind::InvalidData && actual == path));
            assert_eq!(marker_bytes(directory.path()), marker_before);
            assert_eq!(journals(directory.path()), before);
        }
    }

    #[test]
    fn both_marker_consumers_preserve_marker_if_journal_was_truncated_after_counts() {
        let (directory, mut store, hashes) = fixture(2);
        write_marker(&store, &hashes);
        let path = directory.path().join(HASHES_FILE_NAME);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(63)
            .unwrap();
        let before = journals(directory.path());
        let marker_before = marker_bytes(directory.path());
        // The caller's counts were read before truncation. Both consumers must
        // propagate the real descriptor's range refusal, not authorize repair.
        for repair in [false, true] {
            set_hash_cursor(&mut store, 7);
            let result = if repair {
                store.validated_verified_snapshot_tail(2, 2)
            } else {
                store.validated_verified_snapshot_tail_read_only(2, 2)
            };
            assert_range_error(result.unwrap_err(), 0, 2);
            assert_eq!(hash_cursor(&mut store), 7);
            assert_eq!(marker_bytes(directory.path()), marker_before);
            assert_eq!(journals(directory.path()), before);
        }
    }

    #[test]
    fn malformed_marker_still_distinguishes_read_only_and_repair_consumers() {
        let (directory, mut store, _) = fixture(2);
        let path = directory.path().join(VERIFIED_SNAPSHOT_TAIL_FILE_NAME);
        std::fs::write(&path, b"malformed snapshot-tail marker").unwrap();
        let before = journals(directory.path());
        let marker_before = marker_bytes(directory.path());
        assert!(matches!(
            store.validated_verified_snapshot_tail_read_only(2, 2),
            Err(Error::InvalidSnapshotBootstrapMarker { path: actual, .. }) if actual == path
        ));
        assert_eq!(marker_bytes(directory.path()), marker_before);
        assert_eq!(journals(directory.path()), before);
        assert!(
            store
                .validated_verified_snapshot_tail(2, 2)
                .unwrap()
                .is_none()
        );
        assert!(!path.exists());
        assert_eq!(journals(directory.path()), before);
    }
}
