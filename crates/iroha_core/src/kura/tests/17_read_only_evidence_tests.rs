// Read-only disk admission tests. The structural merge fixture is not a finality proof.
#[cfg(unix)]
mod canonical_evidence_reader_tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};

    struct Fixture {
        _directory: tempfile::TempDir,
        root: PathBuf,
        merge: PathBuf,
        blocks: Vec<Arc<SignedBlock>>,
        entries: Vec<MergeLedgerEntry>,
    }
    impl Fixture {
        fn new(with_merge: bool) -> Self {
            let directory = tempfile::tempdir().expect("fixture directory");
            let root = directory
                .path()
                .canonicalize()
                .expect("absolute real directory");
            let genesis = retained_archive_empty_block(None);
            let second = retained_archive_empty_block(Some(&genesis));
            let (second, entries) = if with_merge {
                let mut entry = sample_merge_entry_for_block(1, &second);
                (bind_merge_entry_to_carrier(second, &mut entry), vec![entry])
            } else {
                (second, Vec::new())
            };
            let third = retained_archive_empty_block(Some(&second));
            let fixture = Self {
                _directory: directory,
                merge: root.join("merge.log"),
                root,
                blocks: vec![genesis, second, third],
                entries,
            };
            fixture.write_store();
            fixture.write_log(&fixture.entries);
            fixture
        }
        fn write_store(&self) {
            let mut data = Vec::new();
            let mut indices = Vec::new();
            let mut hashes = Vec::new();
            for block in &self.blocks {
                let wire = block.encode_wire().expect("actual canonical block wire");
                indices.extend_from_slice(
                    &BlockIndex {
                        start: data.len() as u64,
                        length: wire.len() as u64,
                    }
                    .encode(),
                );
                hashes.extend_from_slice(block.hash().as_ref());
                data.extend_from_slice(&wire);
            }
            fs::write(self.root.join(DATA_FILE_NAME), data).expect("data");
            fs::write(self.root.join(INDEX_FILE_NAME), indices).expect("index");
            fs::write(self.root.join(HASHES_FILE_NAME), hashes).expect("hashes");
            self.write_marker(BlockStoreCommitMarker::new(
                self.blocks.len() as u64,
                self.blocks.last().map(|b| b.hash()),
            ));
        }
        fn write_marker(&self, marker: BlockStoreCommitMarker) {
            fs::write(
                self.root.join(COUNT_FILE_NAME),
                norito::encode_canonical(&marker).expect("canonical marker"),
            )
            .expect("marker");
        }
        fn write_log(&self, entries: &[MergeLedgerEntry]) {
            let mut bytes = Vec::new();
            for entry in entries {
                let payload = entry.encode();
                bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                bytes.extend_from_slice(&payload);
            }
            fs::write(&self.merge, bytes).expect("exact stored merge frames");
        }
        fn limits(&self) -> CanonicalKuraEvidenceLimits {
            CanonicalKuraEvidenceLimits {
                first_height: 1,
                last_height: 3,
                max_committed_blocks: 10,
                max_store_data_bytes: 2 * 1024 * 1024,
                max_carrier_bytes: 1024 * 1024,
                max_merge_log_bytes: 2 * 1024 * 1024,
                max_merge_frames: 10,
                max_output_bytes: 8 * 1024 * 1024,
                max_decode_allocation_bytes: 64 * 1024 * 1024,
                owner_uid: fs::metadata(&self.root).expect("owner").uid(),
            }
        }
        fn open(&self) -> CanonicalKuraEvidenceReader {
            CanonicalKuraEvidenceReader::open(&self.root, &self.merge, self.limits())
                .expect("clean fixture admission")
        }
        fn requests(&self) -> Vec<CanonicalKuraMergeRequest> {
            self.entries
                .iter()
                .map(|entry| CanonicalKuraMergeRequest {
                    carrier_height: entry.merge_qc.carrier_height,
                    reference: CertifiedMergeLedgerReference::new(entry),
                })
                .collect()
        }
        fn read_all(&self, reader: &mut CanonicalKuraEvidenceReader) -> u64 {
            let mut size = 0;
            for (index, block) in self.blocks.iter().enumerate() {
                let wire = reader
                    .read_carrier(index as u64 + 1)
                    .expect("complete next carrier");
                assert_eq!(wire, block.encode_wire().expect("canonical expected"));
                size += wire.len() as u64;
            }
            size
        }
        fn files(&self) -> Vec<PathBuf> {
            [
                DATA_FILE_NAME,
                INDEX_FILE_NAME,
                HASHES_FILE_NAME,
                COUNT_FILE_NAME,
                "merge.log",
            ]
            .map(|name| self.root.join(name))
            .to_vec()
        }
        fn snapshot(&self) -> Vec<(Vec<u8>, u32, i64, i64, i64, i64, u64)> {
            self.files()
                .iter()
                .map(|path| {
                    let metadata = fs::metadata(path).expect("metadata");
                    (
                        fs::read(path).expect("bytes"),
                        metadata.mode(),
                        metadata.mtime(),
                        metadata.mtime_nsec(),
                        metadata.ctime(),
                        metadata.ctime_nsec(),
                        metadata.ino(),
                    )
                })
                .collect()
        }
    }

    #[test]
    fn read_only_evidence_completes_both_orders_without_changing_read_only_files() {
        for scan_first in [false, true] {
            let fixture = Fixture::new(true);
            for path in fixture.files() {
                fs::set_permissions(path, fs::Permissions::from_mode(0o400))
                    .expect("read-only source");
            }
            let before = fixture.snapshot();
            let mut reader = fixture.open();
            let mut seen = Vec::new();
            let mut consume = |height, entry: &MergeLedgerEntry, bytes: &[u8]| {
                assert_eq!(height, 2);
                assert_eq!(entry, &fixture.entries[0]);
                assert_eq!(
                    bytes,
                    norito::encode_canonical(entry).expect("canonical frame")
                );
                assert_ne!(bytes, entry.encode());
                seen.push(bytes.len() as u64);
                Ok(())
            };
            if scan_first {
                reader
                    .scan_merge_entries(&fixture.requests(), &mut consume)
                    .expect("scan first");
            }
            let carrier_bytes = fixture.read_all(&mut reader);
            if !scan_first {
                reader
                    .scan_merge_entries(&fixture.requests(), &mut consume)
                    .expect("scan last");
            }
            let complete = reader.finish().expect("both gates complete");
            assert_eq!(complete.committed_height(), 3);
            assert_eq!(complete.carrier_count(), 3);
            assert_eq!(complete.merge_frames(), 1);
            assert_eq!(
                complete.output_bytes(),
                carrier_bytes + seen.iter().sum::<u64>()
            );
            assert_eq!(seen.len(), 1);
            assert_eq!(fixture.snapshot(), before);
        }
    }
    #[test]
    fn read_only_evidence_zero_requests_still_requires_complete_scan_and_carriers() {
        let fixture = Fixture::new(false);
        let mut reader = fixture.open();
        fixture.read_all(&mut reader);
        assert!(reader.finish().is_err());
        let mut reader = fixture.open();
        reader
            .scan_merge_entries(&[], |_, _, _| panic!("empty log has no callback"))
            .expect("complete empty scan");
        assert!(reader.finish().is_err());
        let mut reader = fixture.open();
        fixture.read_all(&mut reader);
        reader
            .scan_merge_entries(&[], |_, _, _| panic!("empty log has no callback"))
            .expect("empty scan");
        assert_eq!(reader.finish().expect("complete").merge_frames(), 0);
        fs::write(&fixture.merge, [1, 2, 3]).expect("partial tail");
        let mut reader = fixture.open();
        assert!(reader.scan_merge_entries(&[], |_, _, _| Ok(())).is_err());
        assert!(reader.finish().is_err());
    }
    #[test]
    fn read_only_evidence_rejects_skips_repeats_extra_reads_and_duplicate_scan() {
        let fixture = Fixture::new(false);
        for requested in [0, 2, 4, u64::MAX] {
            let mut reader = fixture.open();
            assert!(reader.read_carrier(requested).is_err());
            assert!(reader.read_carrier(1).is_err());
            assert!(reader.finish().is_err());
        }
        let mut reader = fixture.open();
        reader.read_carrier(1).expect("first");
        assert!(reader.read_carrier(1).is_err());
        assert!(reader.finish().is_err());
        let mut reader = fixture.open();
        fixture.read_all(&mut reader);
        assert!(reader.read_carrier(4).is_err());
        assert!(reader.finish().is_err());
        let mut reader = fixture.open();
        reader
            .scan_merge_entries(&[], |_, _, _| Ok(()))
            .expect("first scan");
        assert!(reader.scan_merge_entries(&[], |_, _, _| Ok(())).is_err());
        assert!(reader.finish().is_err());
    }
    #[test]
    fn read_only_evidence_callback_error_and_caught_panic_poison_finish() {
        let fixture = Fixture::new(true);
        let mut reader = fixture.open();
        fixture.read_all(&mut reader);
        assert!(
            reader
                .scan_merge_entries(&fixture.requests(), |_, _, _| Err(
                    CanonicalKuraEvidenceError::Invalid("consumer rejection")
                ))
                .is_err()
        );
        assert!(reader.finish().is_err());
        let mut reader = fixture.open();
        fixture.read_all(&mut reader);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            reader.scan_merge_entries(&fixture.requests(), |_, _, _| panic!("consumer panic"))
        }));
        assert!(caught.is_err());
        assert!(
            reader
                .scan_merge_entries(&fixture.requests(), |_, _, _| Ok(()))
                .is_err()
        );
        assert!(reader.finish().is_err());
    }
    #[test]
    fn read_only_evidence_all_five_sources_are_bound_through_consuming_finish() {
        for source in 0..5 {
            let fixture = Fixture::new(true);
            let mut reader = fixture.open();
            fixture.read_all(&mut reader);
            reader
                .scan_merge_entries(&fixture.requests(), |_, _, _| Ok(()))
                .expect("scan");
            let path = &fixture.files()[source];
            let bytes = fs::read(path).expect("preimage");
            fs::rename(path, path.with_extension("old")).expect("replace admitted inode");
            fs::write(path, bytes).expect("identical new inode");
            assert!(reader.finish().is_err(), "source {source}");
        }
    }
    #[test]
    fn read_only_evidence_catches_changes_inside_consumer_before_scan_success() {
        let fixture = Fixture::new(true);
        let mut reader = fixture.open();
        let result = reader.scan_merge_entries(&fixture.requests(), |_, _, _| {
            let path = fixture.root.join(COUNT_FILE_NAME);
            let mut bytes = fs::read(&path).expect("marker");
            bytes.push(0);
            fs::write(path, bytes).expect("marker changed during callback");
            Ok(())
        });
        assert!(result.is_err());
        assert!(reader.finish().is_err());
    }
    #[test]
    fn read_only_evidence_rejects_replaced_parent_and_permission_drift() {
        for change_mode in [false, true] {
            let fixture = Fixture::new(false);
            let mut reader = fixture.open();
            fixture.read_all(&mut reader);
            reader
                .scan_merge_entries(&[], |_, _, _| Ok(()))
                .expect("scan");
            if change_mode {
                fs::set_permissions(
                    fixture.root.join(DATA_FILE_NAME),
                    fs::Permissions::from_mode(0o400),
                )
                .expect("mode change");
            } else {
                let nested = fixture.root.join("moved");
                fs::create_dir(&nested).expect("alternate");
                fs::rename(&fixture.merge, nested.join("merge.log")).expect("move original");
                fs::write(&fixture.merge, []).expect("replacement");
            }
            assert!(reader.finish().is_err());
        }
        let parent = tempfile::tempdir().expect("outer");
        let store = parent.path().join("store");
        fs::create_dir(&store).expect("store");
        let fixture = Fixture::new(false);
        for path in fixture.files() {
            fs::copy(&path, store.join(path.file_name().expect("name"))).expect("copy");
        }
        let store = store.canonicalize().expect("real path");
        let mut reader =
            CanonicalKuraEvidenceReader::open(&store, &store.join("merge.log"), fixture.limits())
                .expect("admit nested");
        fs::rename(&store, store.with_extension("old")).expect("parent replacement");
        fs::create_dir(&store).expect("new named parent");
        assert!(reader.read_carrier(1).is_err());
        assert!(reader.finish().is_err());
    }
    #[test]
    fn read_only_evidence_does_not_create_missing_paths_or_admit_aliases() {
        let fixture = Fixture::new(false);
        let missing = fixture.root.join("missing");
        assert!(
            CanonicalKuraEvidenceReader::open(
                &missing,
                &missing.join("merge.log"),
                fixture.limits()
            )
            .is_err()
        );
        assert!(!missing.exists());
        assert!(
            CanonicalKuraEvidenceReader::open(
                &fixture.root,
                &fixture.root.join(DATA_FILE_NAME),
                fixture.limits()
            )
            .is_err()
        );
        fs::remove_file(&fixture.merge).expect("remove");
        assert!(
            CanonicalKuraEvidenceReader::open(&fixture.root, &fixture.merge, fixture.limits())
                .is_err()
        );
        assert!(!fixture.merge.exists());
    }
    #[test]
    fn read_only_evidence_rejects_symlink_hardlink_fifo_and_post_admission_replacement() {
        for kind in 0..4 {
            let fixture = Fixture::new(false);
            let original = fixture.root.join("original");
            fs::rename(&fixture.merge, &original).expect("save file");
            match kind {
                0 => symlink(&original, &fixture.merge).expect("symlink"),
                1 => fs::hard_link(&original, &fixture.merge).expect("hardlink"),
                2 => {
                    drop(resource_file_fifo_with_live_peer(&fixture.merge));
                }
                _ => {
                    fs::create_dir(&fixture.merge).expect("directory");
                }
            }
            assert!(
                CanonicalKuraEvidenceReader::open(&fixture.root, &fixture.merge, fixture.limits())
                    .is_err(),
                "kind {kind}"
            );
        }
        for fifo in [false, true] {
            let fixture = Fixture::new(false);
            let mut peer = None;
            let result = CanonicalKuraEvidenceReader::open_after_admission(
                &fixture.root,
                &fixture.merge,
                fixture.limits(),
                |path| {
                    if path != fixture.merge {
                        return;
                    }
                    fs::remove_file(path).expect("replace after metadata");
                    if fifo {
                        peer = Some(resource_file_fifo_with_live_peer(path));
                    } else {
                        fs::write(path, []).expect("same bytes new inode");
                    }
                },
            );
            assert!(result.is_err());
            drop(peer);
        }
    }
    #[test]
    fn read_only_evidence_rejects_noncanonical_marker_and_committed_prefix_corruption() {
        for variant in 0..11 {
            let fixture = Fixture::new(false);
            match variant {
                0 => fixture.write_marker(BlockStoreCommitMarker {
                    version: 2,
                    count: 3,
                    tip_hash: Some(fixture.blocks[2].hash()),
                }),
                1 => fixture.write_marker(BlockStoreCommitMarker::new(0, None)),
                2 => fixture.write_marker(BlockStoreCommitMarker::new(
                    2,
                    Some(fixture.blocks[1].hash()),
                )),
                3 => fixture.write_marker(BlockStoreCommitMarker::new(
                    3,
                    Some(fixture.blocks[0].hash()),
                )),
                4 => {
                    let path = fixture.root.join(COUNT_FILE_NAME);
                    let mut bytes = fs::read(&path).expect("marker");
                    bytes.push(0);
                    fs::write(path, bytes).expect("tail");
                }
                5 | 6 => {
                    let path = fixture.root.join(if variant == 5 {
                        INDEX_FILE_NAME
                    } else {
                        HASHES_FILE_NAME
                    });
                    let mut bytes = fs::read(&path).expect("journal");
                    bytes.pop();
                    fs::write(path, bytes).expect("partial record");
                }
                7 => {
                    fs::OpenOptions::new()
                        .append(true)
                        .open(fixture.root.join(DATA_FILE_NAME))
                        .expect("data")
                        .write_all(&[0])
                        .expect("uncommitted suffix");
                }
                8 => {
                    let path = fixture.root.join(INDEX_FILE_NAME);
                    let mut bytes = fs::read(&path).expect("index");
                    bytes[..8].copy_from_slice(&1_u64.to_le_bytes());
                    fs::write(path, bytes).expect("gap");
                }
                9 => {
                    let path = fixture.root.join(INDEX_FILE_NAME);
                    let mut bytes = fs::read(&path).expect("index");
                    bytes[..8].copy_from_slice(&EVICTED_BLOCK_START.to_le_bytes());
                    fs::write(path, bytes).expect("evicted");
                }
                _ => {
                    let path = fixture.root.join(HASHES_FILE_NAME);
                    let mut bytes = fs::read(&path).expect("hashes");
                    bytes[31] &= !1;
                    fs::write(path, bytes).expect("noncanonical hash");
                }
            }
            assert!(
                CanonicalKuraEvidenceReader::open(&fixture.root, &fixture.merge, fixture.limits())
                    .is_err(),
                "variant {variant}"
            );
        }
    }
    #[test]
    fn read_only_evidence_carrier_decode_hash_and_parent_failures_poison() {
        for variant in 0..3 {
            let mut fixture = Fixture::new(false);
            if variant == 0 {
                let path = fixture.root.join(DATA_FILE_NAME);
                let mut bytes = fs::read(&path).expect("data");
                bytes[0] = 255;
                fs::write(path, bytes).expect("version");
            }
            if variant == 1 {
                let path = fixture.root.join(HASHES_FILE_NAME);
                let mut bytes = fs::read(&path).expect("hashes");
                bytes[0] ^= 2;
                fs::write(path, bytes).expect("wrong first hash, same marker tip");
            }
            if variant == 2 {
                fixture.blocks[1] = retained_archive_empty_block(None);
                fixture.write_store();
            }
            let mut reader = fixture.open();
            if variant == 2 {
                reader.read_carrier(1).expect("unchanged first carrier");
                assert!(reader.read_carrier(2).is_err());
            } else {
                assert!(reader.read_carrier(1).is_err());
            }
            assert!(reader.finish().is_err());
        }
    }
    #[test]
    fn read_only_evidence_full_merge_scan_rejects_frame_tail_version_and_order() {
        for variant in 0..8 {
            let fixture = Fixture::new(true);
            match variant {
                0 => fs::write(&fixture.merge, [0, 0, 0, 0]).expect("zero frame"),
                1 => fs::write(
                    &fixture.merge,
                    ((MAX_MERGE_LEDGER_ENTRY_BYTES + 1) as u32).to_le_bytes(),
                )
                .expect("oversize declaration"),
                2 => {
                    let mut bytes = fs::read(&fixture.merge).expect("log");
                    bytes.pop();
                    fs::write(&fixture.merge, bytes).expect("short payload");
                }
                3 => {
                    let mut bytes = fs::read(&fixture.merge).expect("log");
                    bytes.push(1);
                    fs::write(&fixture.merge, bytes).expect("trailing prefix fragment");
                }
                4 => {
                    let mut entry = fixture.entries[0].clone();
                    entry.version = 0;
                    fixture.write_log(&[entry]);
                }
                5 => {
                    let mut entry = fixture.entries[0].clone();
                    entry.epoch_id = 2;
                    fixture.write_log(&[entry]);
                }
                6 => fixture.write_log(&[fixture.entries[0].clone(), fixture.entries[0].clone()]),
                _ => {
                    let mut entry = fixture.entries[0].clone();
                    entry.merge_qc.carrier_height = 4;
                    fixture.write_log(&[entry]);
                }
            }
            let mut reader = fixture.open();
            assert!(
                reader
                    .scan_merge_entries(&fixture.requests(), |_, _, _| Ok(()))
                    .is_err(),
                "variant {variant}"
            );
            assert!(reader.finish().is_err());
        }
    }
    #[test]
    fn read_only_evidence_requires_exact_requested_and_actual_carrier_references() {
        for variant in 0..5 {
            let fixture = Fixture::new(true);
            let mut requests = fixture.requests();
            match variant {
                0 => requests.clear(),
                1 => requests.push(fixture.requests().remove(0)),
                2 => requests[0].carrier_height = 1,
                3 => {
                    requests[0].reference.entry_hash =
                        HashOf::from_untyped_unchecked(Hash::new(b"wrong entry"))
                }
                _ => requests[0].reference.encoded_len += 1,
            }
            let mut reader = fixture.open();
            assert!(
                reader
                    .scan_merge_entries(&requests, |_, _, _| Ok(()))
                    .is_err(),
                "variant {variant}"
            );
            assert!(reader.finish().is_err());
        }
        let fixture = Fixture::new(false);
        let merge_fixture = Fixture::new(true);
        fixture.write_log(&merge_fixture.entries);
        let mut reader = fixture.open();
        reader
            .scan_merge_entries(&merge_fixture.requests(), |_, _, _| Ok(()))
            .expect("independently matched log/reference is provisional");
        fixture.read_all(&mut reader);
        assert!(
            reader.finish().is_err(),
            "actual carriers did not carry supplied references"
        );
        let fixture = Fixture::new(true);
        fixture.write_log(&[]);
        let mut reader = fixture.open();
        fixture.read_all(&mut reader);
        reader
            .scan_merge_entries(&[], |_, _, _| Ok(()))
            .expect("complete empty log is provisional");
        assert!(
            reader.finish().is_err(),
            "actual carrier reference is missing from log"
        );
    }
    #[test]
    fn read_only_evidence_applies_independent_size_frame_owner_and_decode_bounds() {
        for variant in 0..10 {
            let fixture = Fixture::new(true);
            let mut limits = fixture.limits();
            match variant {
                0 => limits.first_height = 0,
                1 => limits.last_height = 4,
                2 => limits.max_committed_blocks = 1_000_001,
                3 => limits.max_store_data_bytes = 1,
                4 => limits.max_merge_log_bytes = 1,
                5 => limits.owner_uid ^= 1,
                6 => limits.max_carrier_bytes = 32 * 1024 * 1024 + 1,
                7 => limits.max_output_bytes = 0,
                8 => limits.max_decode_allocation_bytes = 0,
                _ => limits.max_merge_frames = limits.max_committed_blocks + 1,
            }
            assert!(
                CanonicalKuraEvidenceReader::open(&fixture.root, &fixture.merge, limits).is_err(),
                "variant {variant}"
            );
        }
        let fixture = Fixture::new(true);
        for variant in 0..4 {
            let mut limits = fixture.limits();
            match variant {
                0 => limits.max_carrier_bytes = 1,
                1 => limits.max_output_bytes = 1,
                2 => limits.max_merge_frames = 0,
                _ => limits.max_decode_allocation_bytes = 1,
            }
            let result = CanonicalKuraEvidenceReader::open(&fixture.root, &fixture.merge, limits);
            if variant == 3 {
                assert!(result.is_err(), "marker allocation alone exceeds one byte");
                continue;
            }
            let mut reader = result.expect("bounds admit fixed prefix");
            if variant == 2 {
                assert!(
                    reader
                        .scan_merge_entries(&fixture.requests(), |_, _, _| Ok(()))
                        .is_err()
                );
            } else {
                assert!(reader.read_carrier(1).is_err());
            }
            assert!(reader.finish().is_err());
        }
    }
    #[test]
    fn read_only_evidence_exact_returned_byte_budget_succeeds_and_one_less_fails() {
        let fixture = Fixture::new(true);
        let exact = fixture
            .blocks
            .iter()
            .map(|block| block.encode_wire().expect("wire").len() as u64)
            .sum::<u64>()
            + norito::encode_canonical(&fixture.entries[0])
                .expect("entry")
                .len() as u64;
        for deficit in [0, 1] {
            let mut limits = fixture.limits();
            limits.max_output_bytes = exact - deficit;
            let mut reader =
                CanonicalKuraEvidenceReader::open(&fixture.root, &fixture.merge, limits)
                    .expect("admit exact budget");
            fixture.read_all(&mut reader);
            let result = reader.scan_merge_entries(&fixture.requests(), |_, _, _| Ok(()));
            if deficit == 0 {
                result.expect("exact bytes accepted");
                assert_eq!(
                    reader
                        .finish()
                        .expect("complete exact budget")
                        .output_bytes(),
                    exact
                );
            } else {
                assert!(result.is_err());
                assert!(reader.finish().is_err());
            }
        }
    }
    #[test]
    fn read_only_evidence_requested_subinterval_still_scans_all_prior_epochs() {
        let fixture = Fixture::new(true);
        let mut limits = fixture.limits();
        limits.first_height = 3;
        let mut reader = CanonicalKuraEvidenceReader::open(&fixture.root, &fixture.merge, limits)
            .expect("subinterval");
        assert_eq!(
            reader.read_carrier(3).expect("last carrier"),
            fixture.blocks[2].encode_wire().expect("wire")
        );
        reader
            .scan_merge_entries(&[], |_, _, _| panic!("outside interval"))
            .expect("full prior epoch scan");
        let complete = reader.finish().expect("subinterval qualified");
        assert_eq!(complete.carrier_count(), 1);
        assert_eq!(complete.merge_frames(), 1);
        let mut broken = fixture.entries[0].clone();
        broken.epoch_id = 2;
        fixture.write_log(&[broken]);
        let mut reader = CanonicalKuraEvidenceReader::open(&fixture.root, &fixture.merge, limits)
            .expect("same valid journals");
        assert!(
            reader.scan_merge_entries(&[], |_, _, _| Ok(())).is_err(),
            "unrequested old epoch still validated"
        );
        assert!(reader.finish().is_err());
    }
    #[test]
    fn read_only_evidence_actual_handles_are_nonblocking_close_on_exec_and_read_only() {
        let fixture = Fixture::new(false);
        let reader = fixture.open();
        for source in [
            &reader.sources.data,
            &reader.sources.index,
            &reader.sources.hashes,
            &reader.sources.marker,
            &reader.sources.merge,
        ] {
            let (flags, descriptor_flags) = source.flags_for_test().expect("actual retained flags");
            assert_eq!(
                flags & rustix::fs::OFlags::ACCMODE,
                rustix::fs::OFlags::RDONLY
            );
            assert!(flags.contains(rustix::fs::OFlags::NONBLOCK));
            assert!(descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC));
        }
    }
    #[test]
    fn read_only_evidence_retained_read_rejects_truncation_and_replacement_after_check() {
        for replace in [false, true] {
            let fixture = Fixture::new(false);
            let reader = fixture.open();
            let result = reader
                .sources
                .data
                .read_after_check(0, reader.sources.data.len(), || {
                    let path = fixture.root.join(DATA_FILE_NAME);
                    if replace {
                        let bytes = fs::read(&path).expect("preimage");
                        fs::rename(&path, path.with_extension("old")).expect("old retained inode");
                        fs::write(&path, bytes).expect("same-byte new inode");
                    } else {
                        fs::OpenOptions::new()
                            .write(true)
                            .open(path)
                            .expect("writer")
                            .set_len(0)
                            .expect("truncate");
                    }
                });
            assert!(result.is_err(), "post-check change {replace}");
        }
    }
    #[test]
    fn read_only_evidence_rejects_parent_symlinks_relative_and_non_normal_paths() {
        let fixture = Fixture::new(false);
        let outer = tempfile::tempdir().expect("outer");
        let alias = outer.path().join("alias");
        symlink(&fixture.root, &alias).expect("directory alias");
        assert!(
            CanonicalKuraEvidenceReader::open(&alias, &fixture.merge, fixture.limits()).is_err()
        );
        assert!(
            CanonicalKuraEvidenceReader::open(
                Path::new("relative"),
                &fixture.merge,
                fixture.limits()
            )
            .is_err()
        );
        let dot = fixture.root.join(".");
        assert!(CanonicalKuraEvidenceReader::open(&dot, &fixture.merge, fixture.limits()).is_err());
        let extra = fixture.root.join("..sample").join("..");
        assert!(
            CanonicalKuraEvidenceReader::open(&extra, &fixture.merge, fixture.limits()).is_err()
        );
    }
    #[test]
    fn read_only_evidence_requires_actual_framed_wire_and_rejects_bare_versioned_store() {
        let fixture = Fixture::new(false);
        let mut reader = fixture.open();
        fixture.read_all(&mut reader);
        reader
            .scan_merge_entries(&[], |_, _, _| Ok(()))
            .expect("full empty log");
        assert_eq!(
            reader
                .finish()
                .expect("actual framed positive")
                .carrier_count(),
            3
        );
        let mut data = Vec::new();
        let mut index = Vec::new();
        for block in &fixture.blocks {
            let wire = block
                .canonical_wire()
                .expect("real canonical two representations");
            assert_ne!(wire.as_framed(), wire.as_versioned());
            assert!(
                iroha_data_model::block::decode_versioned_signed_block(wire.as_framed()).is_ok()
            );
            assert!(
                iroha_data_model::block::decode_versioned_signed_block(wire.as_versioned())
                    .is_err()
            );
            let bare = wire.as_versioned();
            index.extend_from_slice(
                &BlockIndex {
                    start: data.len() as u64,
                    length: bare.len() as u64,
                }
                .encode(),
            );
            data.extend_from_slice(bare);
        }
        // Preserve the valid count/tip/hash association and complete contiguous
        // lengths so the negative reaches the actual carrier wire decoder.
        fs::write(fixture.root.join(DATA_FILE_NAME), data).expect("bare data");
        fs::write(fixture.root.join(INDEX_FILE_NAME), index).expect("matching bare lengths");
        let mut reader = fixture.open();
        assert!(matches!(
            reader.read_carrier(1),
            Err(CanonicalKuraEvidenceError::Invalid("carrier decode"))
        ));
        assert!(reader.finish().is_err());
    }
}
