mod block_hash_ranges {
    //! Adversarial controls for bounded canonical hash-journal reads.

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
        (directory, store, hashes)
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
}
