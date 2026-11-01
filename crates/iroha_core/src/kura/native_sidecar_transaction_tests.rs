//! Real native pipeline originals, retry boundaries and rewrite recovery.

use super::*;
use std::fs;

struct Fixture {
    owner: iroha_fs::OwnerDirectory,
    temporary: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let private =
            iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("pipeline")).unwrap();
        let owner = iroha_fs::OwnerDirectory::open(private.path()).unwrap();
        Self { owner, temporary }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.owner.path().join(name)
    }

    fn capture(&self) -> io::Result<NativeSidecarTransaction> {
        let root = self.owner.path();
        let directory = Kura::bind_native_progress_directory(root, root, self.owner.retain()?)
            .map_err(io::Error::other)?;
        NativeSidecarTransaction::capture(BoundProgressNamespace {
            data_path: self.path("progress.norito"),
            index_path: self.path("progress.index"),
            directories: vec![directory],
        })
    }

    fn seed(&self, name: &str, bytes: &[u8]) {
        let mut slot = None;
        self.owner
            .create_private_journal_into(OsStr::new(name), &mut slot)
            .unwrap();
        let mut file = slot.as_ref().unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        self.owner
            .require_same_lock(OsStr::new(name), file)
            .unwrap();
        drop(slot);
        self.owner.sync().unwrap();
    }

    fn seed_pair(&self, data: &str, index: &str, height: u64, payload: &[u8]) {
        self.seed(data, payload);
        let mut bytes = SidecarIndexLayout::base_header(height).to_vec();
        bytes.extend_from_slice(
            &SidecarIndexEntry {
                offset: 0,
                len: payload.len() as u64,
            }
            .to_bytes(),
        );
        self.seed(index, &bytes);
    }

    fn payload(&self, height: u64) -> Vec<u8> {
        let mut index = File::open(self.path("progress.index")).unwrap();
        let length = index.metadata().unwrap().len();
        let layout = SidecarIndexLayout::read_from(&mut index, length).unwrap();
        index
            .seek(SeekFrom::Start(layout.entry_position(height).unwrap()))
            .unwrap();
        let mut bytes = [0; PIPELINE_INDEX_ENTRY_SIZE];
        index.read_exact(&mut bytes).unwrap();
        let entry = SidecarIndexEntry::from_bytes(bytes);
        let mut data = File::open(self.path("progress.norito")).unwrap();
        data.seek(SeekFrom::Start(entry.offset)).unwrap();
        let mut result = vec![0; usize::try_from(entry.len).unwrap()];
        data.read_exact(&mut result).unwrap();
        result
    }
}

struct ResetFaults;

impl Drop for ResetFaults {
    fn drop(&mut self) {
        set_rewrite_index_create_failure_for_tests(false);
        FAIL_NEXT_INDEXED_SIDECAR_INITIAL_DATA_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_INDEXED_SIDECAR_DATA_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_INDEXED_SIDECAR_INDEX_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_SIDECAR_TEMP_MARKER_DIR_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_SIDECAR_PROMOTION_DIR_SYNC.with(|flag| flag.set(false));
    }
}

#[test]
fn native_transaction_append_prepend_overwrite_and_exact_retry_keep_payloads() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    assert!(transaction.matches(
        &fixture.path("progress.norito"),
        &fixture.path("progress.index")
    ));
    assert!(!transaction.matches(
        &fixture.path("other.norito"),
        &fixture.path("progress.index")
    ));
    transaction.append(3, b"three", true).unwrap();
    transaction.append(1, b"one", true).unwrap();
    transaction.append(2, b"two", true).unwrap();
    assert_eq!(fixture.payload(1), b"one");
    assert_eq!(fixture.payload(2), b"two");
    assert_eq!(fixture.payload(3), b"three");
    let before = fs::read(fixture.path("progress.norito")).unwrap();
    transaction.append(2, b"two", true).unwrap();
    assert_eq!(fs::read(fixture.path("progress.norito")).unwrap(), before);
    transaction.append(2, b"replacement", true).unwrap();
    assert_eq!(fixture.payload(1), b"one");
    assert_eq!(fixture.payload(2), b"replacement");
    assert_eq!(fixture.payload(3), b"three");
    assert!(transaction.pending.is_none());
    assert!(transaction.originals.prepend.is_none());
}

#[test]
fn native_transaction_payload_sync_failure_reuses_original_pair_without_duplicate_bytes() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    fail_next_indexed_sidecar_initial_data_sync_for_tests();
    assert!(transaction.append(7, b"payload", true).is_err());
    assert_eq!(
        transaction.pending.as_ref().unwrap().phase,
        AppendPhase::Payload
    );
    let data_id = iroha_fs::FileMetadata::of(original(&transaction.originals.data).unwrap())
        .unwrap()
        .identity();
    let index_id = iroha_fs::FileMetadata::of(original(&transaction.originals.index).unwrap())
        .unwrap()
        .identity();
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .open(fixture.path("progress.norito"))
            .is_err()
    );
    transaction.recover().unwrap();
    assert!(transaction.pending.is_none());
    assert_eq!(fixture.payload(7), b"payload");
    assert_eq!(
        fs::read(fixture.path("progress.norito")).unwrap(),
        b"payload"
    );
    assert_eq!(
        iroha_fs::FileMetadata::of(original(&transaction.originals.data).unwrap())
            .unwrap()
            .identity(),
        data_id
    );
    assert_eq!(
        iroha_fs::FileMetadata::of(original(&transaction.originals.index).unwrap())
            .unwrap()
            .identity(),
        index_id
    );
    transaction.append(7, b"payload", true).unwrap();
    assert_eq!(
        fs::read(fixture.path("progress.norito")).unwrap(),
        b"payload"
    );
}

#[test]
fn native_transaction_published_prepend_retries_barriers_without_renaming_or_cleanup() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(2, b"two", true).unwrap();
    let old_index = iroha_fs::FileMetadata::of(original(&transaction.originals.index).unwrap())
        .unwrap()
        .identity();
    fail_next_indexed_sidecar_dir_sync_for_tests();
    assert!(transaction.append(1, b"one", true).is_err());
    let pending = transaction.pending.as_ref().unwrap();
    assert!(pending.published);
    assert_eq!(pending.phase, AppendPhase::Barriers);
    let published = iroha_fs::FileMetadata::of(original(&transaction.originals.prepend).unwrap())
        .unwrap()
        .identity();
    assert_ne!(published, old_index);
    let payload = fs::read(fixture.path("progress.norito")).unwrap();
    let index = fs::read(fixture.path("progress.index")).unwrap();
    assert_eq!(fixture.payload(1), b"one");
    assert_eq!(fixture.payload(2), b"two");
    fixture.seed("progress.index.prepend.tmp", b"later unrelated original");
    transaction.recover().unwrap();
    assert_eq!(fs::read(fixture.path("progress.norito")).unwrap(), payload);
    assert_eq!(fs::read(fixture.path("progress.index")).unwrap(), index);
    assert_eq!(
        iroha_fs::FileMetadata::of(original(&transaction.originals.index).unwrap())
            .unwrap()
            .identity(),
        published
    );
    assert_eq!(
        fs::read(fixture.path("progress.index.prepend.tmp")).unwrap(),
        b"later unrelated original"
    );
}

#[test]
fn native_transaction_rewrite_marker_failure_keeps_main_pair_byte_exact() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    fixture.seed_pair("progress.norito", "progress.index", 1, b"old");
    fixture.seed_pair(
        "progress.norito.tmp",
        "progress.index.tmp",
        2,
        b"replacement",
    );
    let before = fs::read(fixture.path("progress.index")).unwrap();
    let mut transaction = fixture.capture().unwrap();
    fail_next_sidecar_temp_marker_dir_sync_for_tests();
    assert!(transaction.recover().is_err());
    assert!(!transaction.originals.data_published);
    assert!(!transaction.originals.index_published);
    assert_eq!(fs::read(fixture.path("progress.norito")).unwrap(), b"old");
    assert_eq!(fs::read(fixture.path("progress.index")).unwrap(), before);
    transaction.recover().unwrap();
    assert_eq!(fixture.payload(2), b"replacement");
    assert!(!fixture.path("progress.norito.tmp").exists());
    assert!(!fixture.path("progress.index.tmp").exists());
}

#[test]
fn native_transaction_rewrite_data_publication_failure_retains_latch_and_retries_index() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    fixture.seed_pair("progress.norito", "progress.index", 1, b"old");
    fixture.seed_pair(
        "progress.norito.tmp",
        "progress.index.tmp",
        2,
        b"replacement",
    );
    let old_index = fs::read(fixture.path("progress.index")).unwrap();
    let mut transaction = fixture.capture().unwrap();
    let data_id =
        iroha_fs::FileMetadata::of(original(&transaction.originals.rewrite_data).unwrap())
            .unwrap()
            .identity();
    fail_next_sidecar_promotion_dir_sync_for_tests();
    assert!(transaction.recover().is_err());
    assert!(transaction.originals.data_published);
    assert!(!transaction.originals.index_published);
    assert_eq!(
        fs::read(fixture.path("progress.norito")).unwrap(),
        b"replacement"
    );
    assert_eq!(fs::read(fixture.path("progress.index")).unwrap(), old_index);
    assert!(fixture.path("progress.index.tmp").exists());
    transaction.recover().unwrap();
    assert_eq!(fixture.payload(2), b"replacement");
    assert_eq!(
        iroha_fs::FileMetadata::of(original(&transaction.originals.data).unwrap())
            .unwrap()
            .identity(),
        data_id
    );
    assert!(transaction.originals.rewrite_data.is_none());
    assert!(transaction.originals.rewrite_index.is_none());
}

#[test]
fn native_transaction_fresh_recovery_accepts_index_only_and_canonical_empty_rewrite() {
    for empty in [false, true] {
        let fixture = Fixture::new();
        fixture.seed_pair("progress.norito", "progress.index", 1, b"old");
        if empty {
            fixture.seed("progress.norito.tmp", b"");
            fixture.seed("progress.index.tmp", &SidecarIndexLayout::base_header(1));
        } else {
            let mut bytes = SidecarIndexLayout::base_header(2).to_vec();
            bytes.extend_from_slice(&SidecarIndexEntry { offset: 0, len: 3 }.to_bytes());
            fixture.seed("progress.index.tmp", &bytes);
        }
        let mut transaction = fixture.capture().unwrap();
        transaction.recover().unwrap();
        if empty {
            assert!(
                fs::read(fixture.path("progress.norito"))
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                fs::read(fixture.path("progress.index")).unwrap(),
                SidecarIndexLayout::base_header(1)
            );
        } else {
            assert_eq!(fixture.payload(2), b"old");
        }
        assert!(!fixture.path("progress.index.tmp").exists());
    }
}

#[test]
fn native_transaction_rejects_incomplete_or_invalid_rewrite_without_publication() {
    for kind in [
        "lone-data",
        "headerless-index",
        "out-of-range",
        "partial-pair",
    ] {
        let fixture = Fixture::new();
        if kind == "partial-pair" {
            fixture.seed("progress.norito", b"old");
        } else {
            fixture.seed_pair("progress.norito", "progress.index", 1, b"old");
        }
        match kind {
            "lone-data" => fixture.seed("progress.norito.tmp", b"orphan"),
            "headerless-index" => {
                fixture.seed("progress.index.tmp", &[0; PIPELINE_INDEX_ENTRY_SIZE])
            }
            "out-of-range" => {
                let mut bytes = SidecarIndexLayout::base_header(2).to_vec();
                bytes.extend_from_slice(
                    &SidecarIndexEntry {
                        offset: 100,
                        len: 1,
                    }
                    .to_bytes(),
                );
                fixture.seed("progress.index.tmp", &bytes);
            }
            "partial-pair" => {}
            _ => unreachable!(),
        }
        let before = fs::read(fixture.path("progress.index")).ok();
        let mut transaction = fixture.capture().unwrap();
        assert!(transaction.append(2, b"new", true).is_err(), "{kind}");
        assert_eq!(fs::read(fixture.path("progress.norito")).unwrap(), b"old");
        assert_eq!(fs::read(fixture.path("progress.index")).ok(), before);
    }
}

#[test]
fn native_transaction_refuses_substituted_original_without_touching_either_file() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(1, b"original", true).unwrap();
    let data = fixture.path("progress.norito");
    let displaced = fixture.path("displaced");
    fs::rename(&data, &displaced).unwrap();
    fixture.seed("progress.norito", b"replacement");
    assert!(transaction.append(2, b"must not write", true).is_err());
    assert_eq!(fs::read(data).unwrap(), b"replacement");
    assert_eq!(fs::read(displaced).unwrap(), b"original");
}

#[test]
fn native_transaction_refuses_hardlinks_and_strict_intents_without_removing_them() {
    for kind in ["hardlink", "build", "intent"] {
        let fixture = Fixture::new();
        fixture.seed_pair("progress.norito", "progress.index", 1, b"original");
        let occupied = match kind {
            "hardlink" => {
                let link = fixture.temporary.path().join("alias");
                fs::hard_link(fixture.path("progress.norito"), &link).unwrap();
                link
            }
            "build" => {
                fixture.seed("progress.index.append.build.tmp", b"strict build");
                fixture.path("progress.index.append.build.tmp")
            }
            "intent" => {
                fixture.seed("progress.index.append.intent.tmp", b"strict intent");
                fixture.path("progress.index.append.intent.tmp")
            }
            _ => unreachable!(),
        };
        let before = fs::read(&occupied).unwrap();
        assert!(fixture.capture().is_err());
        assert_eq!(fs::read(occupied).unwrap(), before);
        assert_eq!(
            fs::read(fixture.path("progress.norito")).unwrap(),
            b"original"
        );
    }
}

#[test]
fn native_transaction_refuses_late_private_acl_and_excludes_competing_writer() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(1, b"original", true).unwrap();
    let path = fixture.path("progress.norito");
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    let status = std::process::Command::new("icacls")
        .arg(&path)
        .args(["/grant", "*S-1-1-0:(R)"])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(transaction.append(2, b"refused", true).is_err());
    assert_eq!(fs::read(path).unwrap(), b"original");
}

#[test]
fn native_transaction_rewrite_index_publication_error_never_repeats_either_rename() {
    let fixture = Fixture::new();
    fixture.seed_pair("progress.norito", "progress.index", 1, b"old");
    fixture.seed_pair(
        "progress.norito.tmp",
        "progress.index.tmp",
        2,
        b"replacement",
    );
    let mut transaction = fixture.capture().unwrap();
    let mut steps = Vec::new();
    let error = transaction
        .recover_observed(&BTreeSet::from([2]), &mut |step| {
            steps.push(step);
            if step == RecoveryStep::IndexPublished {
                Err(io::Error::other("after actual index publication"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "after actual index publication");
    assert_eq!(
        steps,
        [RecoveryStep::DataPublished, RecoveryStep::IndexPublished]
    );
    assert!(transaction.originals.data_published);
    assert!(transaction.originals.index_published);
    let data_id =
        iroha_fs::FileMetadata::of(original(&transaction.originals.rewrite_data).unwrap())
            .unwrap()
            .identity();
    let index_id =
        iroha_fs::FileMetadata::of(original(&transaction.originals.rewrite_index).unwrap())
            .unwrap()
            .identity();
    assert_eq!(fixture.payload(2), b"replacement");
    transaction
        .recover_observed(&BTreeSet::from([2]), &mut |_| {
            panic!("published rename must not repeat")
        })
        .unwrap();
    assert_eq!(
        iroha_fs::FileMetadata::of(original(&transaction.originals.data).unwrap())
            .unwrap()
            .identity(),
        data_id
    );
    assert_eq!(
        iroha_fs::FileMetadata::of(original(&transaction.originals.index).unwrap())
            .unwrap()
            .identity(),
        index_id
    );
    assert_eq!(fixture.payload(2), b"replacement");
}

#[test]
fn native_transaction_required_heights_refuse_missing_main_and_rewrite_coverage() {
    for with_rewrite in [false, true] {
        let fixture = Fixture::new();
        fixture.seed_pair("progress.norito", "progress.index", 1, b"old");
        if with_rewrite {
            fixture.seed_pair("progress.norito.tmp", "progress.index.tmp", 2, b"new");
        }
        let before = fs::read(fixture.path("progress.index")).unwrap();
        let mut transaction = fixture.capture().unwrap();
        let required = BTreeSet::from([if with_rewrite { 1 } else { 2 }]);
        assert!(
            transaction
                .recover_observed(&required, &mut |_| panic!(
                    "invalid coverage must not publish"
                ))
                .is_err()
        );
        assert_eq!(fs::read(fixture.path("progress.norito")).unwrap(), b"old");
        assert_eq!(fs::read(fixture.path("progress.index")).unwrap(), before);
        if with_rewrite {
            assert_eq!(
                fs::read(fixture.path("progress.norito.tmp")).unwrap(),
                b"new"
            );
            assert!(fixture.path("progress.index.tmp").exists());
        }
    }
}

#[test]
fn native_transaction_empty_payload_and_late_occupied_absence_preserve_existing_semantics() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(1, b"", true).unwrap();
    assert!(fixture.payload(1).is_empty());
    transaction.append(2, b"nonempty", true).unwrap();
    assert_eq!(fixture.payload(2), b"nonempty");
    drop(transaction);

    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    fixture.seed("progress.norito", b"late occupant");
    assert!(transaction.append(1, b"must not truncate", true).is_err());
    assert_eq!(
        fs::read(fixture.path("progress.norito")).unwrap(),
        b"late occupant"
    );
    assert!(!fixture.path("progress.index").exists());
}

#[test]
fn native_transaction_retry_record_counts_one_resident_association() {
    use resident_inventory::ResidentOwner;
    let fixture = Fixture::new();
    let mut slot: Option<NativeSidecarTransaction> = None;
    assert_eq!(slot.resident_associations().unwrap(), 0);
    slot = Some(fixture.capture().unwrap());
    assert_eq!(slot.resident_associations().unwrap(), 1);
    let _reset = ResetFaults;
    fail_next_indexed_sidecar_initial_data_sync_for_tests();
    assert!(
        slot.as_mut()
            .unwrap()
            .append(1, b"retained retry", true)
            .is_err()
    );
    assert_eq!(slot.resident_associations().unwrap(), 1);
    assert!(slot.resident_complete());
    slot.as_mut().unwrap().recover().unwrap();
    slot = None;
    assert_eq!(slot.resident_associations().unwrap(), 0);
}

#[test]
fn native_pipeline_child_uses_original_parent_and_rejects_unsafe_existing_entry() {
    let fixture = Fixture::new();
    let child = ensure_pipeline_child(&fixture.owner).unwrap();
    let identity = child.identity().unwrap();
    drop(child);
    assert_eq!(
        ensure_pipeline_child(&fixture.owner)
            .unwrap()
            .identity()
            .unwrap(),
        identity
    );
    ensure_pipeline_child(&fixture.owner)
        .unwrap()
        .remove_empty()
        .unwrap();
    fixture.seed(PIPELINE_DIR_NAME, b"not a directory");
    assert!(ensure_pipeline_child(&fixture.owner).is_err());
    assert_eq!(
        fs::read(fixture.path(PIPELINE_DIR_NAME)).unwrap(),
        b"not a directory"
    );
}

#[test]
fn native_transaction_prepend_build_failure_precedes_any_payload_append() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(2, b"two", true).unwrap();
    let before = fs::read(fixture.path("progress.norito")).unwrap();
    FAIL_NEXT_INDEXED_SIDECAR_INDEX_SYNC.with(|flag| flag.set(true));
    assert!(transaction.append(1, b"one", true).is_err());
    assert_eq!(
        transaction.pending.as_ref().unwrap().phase,
        AppendPhase::PrependIndex
    );
    assert_eq!(fs::read(fixture.path("progress.norito")).unwrap(), before);
    transaction.recover().unwrap();
    assert_eq!(fixture.payload(1), b"one");
    assert_eq!(fixture.payload(2), b"two");
}

#[test]
fn native_transaction_stale_prepend_retires_exact_original_after_validating_main() {
    let fixture = Fixture::new();
    fixture.seed_pair("progress.norito", "progress.index", 1, b"main");
    fixture.seed("progress.index.prepend.tmp", b"unpublished residue");
    let mut transaction = fixture.capture().unwrap();
    transaction.recover().unwrap();
    assert!(!fixture.path("progress.index.prepend.tmp").exists());
    assert!(transaction.originals.prepend.is_none());
    assert!(!transaction.originals.prepend_disposed);
    assert_eq!(fixture.payload(1), b"main");
}

#[test]
fn native_transaction_rejected_gap_does_not_block_the_next_valid_request() {
    for backward in [false, true] {
        let fixture = Fixture::new();
        let mut transaction = fixture.capture().unwrap();
        let first = if backward {
            MAX_INDEXED_SIDECAR_GAP_ENTRIES + 2
        } else {
            1
        };
        transaction.append(first, b"first", true).unwrap();
        let data_id =
            iroha_fs::FileIdentity::of(original(&transaction.originals.data).unwrap()).unwrap();
        let index_id =
            iroha_fs::FileIdentity::of(original(&transaction.originals.index).unwrap()).unwrap();
        let before_data = fs::read(fixture.path("progress.norito")).unwrap();
        let before_index = fs::read(fixture.path("progress.index")).unwrap();
        let rejected = if backward {
            1
        } else {
            first + MAX_INDEXED_SIDECAR_GAP_ENTRIES + 2
        };
        assert!(
            transaction
                .append(rejected, b"must not persist", true)
                .is_err()
        );
        assert!(transaction.pending.is_none());
        assert_eq!(
            fs::read(fixture.path("progress.norito")).unwrap(),
            before_data
        );
        assert_eq!(
            fs::read(fixture.path("progress.index")).unwrap(),
            before_index
        );
        assert!(!fixture.path("progress.index.prepend.tmp").exists());
        transaction.append(first + 1, b"next", true).unwrap();
        assert_eq!(fixture.payload(first), b"first");
        assert_eq!(fixture.payload(first + 1), b"next");
        assert_eq!(
            iroha_fs::FileIdentity::of(original(&transaction.originals.data).unwrap()).unwrap(),
            data_id
        );
        assert_eq!(
            iroha_fs::FileIdentity::of(original(&transaction.originals.index).unwrap()).unwrap(),
            index_id
        );
    }
}

#[test]
fn native_rewrite_creator_retry_keeps_original_and_resets_its_cursor() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(1, b"old", true).unwrap();
    transaction.append(2, b"retained", true).unwrap();
    let policy = IndexedSidecarRewrite::Newest {
        retention: NonZeroUsize::new(1).unwrap(),
        pinned_height: None,
    };
    set_rewrite_index_create_failure_for_tests(true);
    assert!(transaction.rewrite(policy, "creator retry").is_err());
    assert_eq!(
        transaction.rewrite.as_ref().unwrap().phase,
        RewritePhase::Building
    );
    let retained_id =
        iroha_fs::FileIdentity::of(original(&transaction.originals.rewrite_data).unwrap()).unwrap();
    let mut original_data = original(&transaction.originals.rewrite_data)
        .unwrap()
        .try_clone()
        .unwrap();
    original_data
        .write_all(b"partial creator bytes at a nonzero cursor")
        .unwrap();
    assert!(
        transaction.recover().is_err(),
        "partial creator is not a committed marker"
    );
    assert!(
        transaction
            .rewrite(
                IndexedSidecarRewrite::Newest {
                    retention: NonZeroUsize::new(2).unwrap(),
                    pinned_height: None,
                },
                "changed request"
            )
            .is_err()
    );
    assert_eq!(
        iroha_fs::FileIdentity::of(&original_data).unwrap(),
        retained_id
    );
    assert_eq!(
        fs::read(fixture.path("progress.norito")).unwrap(),
        b"oldretained"
    );
    transaction.rewrite(policy, "creator retry").unwrap();
    assert_eq!(
        iroha_fs::FileIdentity::of(original(&transaction.originals.data).unwrap()).unwrap(),
        retained_id
    );
    assert_eq!(
        fs::read(fixture.path("progress.norito")).unwrap(),
        b"retained"
    );
    assert_eq!(fixture.payload(2), b"retained");
    assert!(transaction.rewrite.is_none());
}

#[test]
fn native_rewrite_creator_ready_retry_keeps_required_evidence_validation() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    for height in 1..=3 {
        transaction
            .append(height, &[u8::try_from(height).unwrap()], true)
            .unwrap();
    }
    let required = BTreeSet::from([1]);
    fail_next_sidecar_temp_marker_dir_sync_for_tests();
    assert!(
        transaction
            .rewrite(
                IndexedSidecarRewrite::AfterTerminalFrontier {
                    terminal_height: 3,
                    retention: NonZeroUsize::new(1).unwrap(),
                    required_heights: &required,
                },
                "required creator"
            )
            .is_err()
    );
    assert_eq!(
        transaction.rewrite.as_ref().unwrap().phase,
        RewritePhase::Ready
    );
    let mut index = original(&transaction.originals.rewrite_index)
        .unwrap()
        .try_clone()
        .unwrap();
    let length = index.metadata().unwrap().len();
    let layout = SidecarIndexLayout::read_from(&mut index, length).unwrap();
    let position = layout.entry_position(1).unwrap();
    index.seek(SeekFrom::Start(position)).unwrap();
    let mut admitted = [0; PIPELINE_INDEX_ENTRY_SIZE];
    index.read_exact(&mut admitted).unwrap();
    index.seek(SeekFrom::Start(position)).unwrap();
    index.write_all(&[0; PIPELINE_INDEX_ENTRY_SIZE]).unwrap();
    index.sync_all().unwrap();
    assert!(
        transaction.recover().is_err(),
        "an empty caller set cannot drop the retained requirement"
    );
    assert_eq!(
        fs::read(fixture.path("progress.norito")).unwrap(),
        [1, 2, 3]
    );
    index.seek(SeekFrom::Start(position)).unwrap();
    index.write_all(&admitted).unwrap();
    index.sync_all().unwrap();
    drop(index);
    transaction.recover().unwrap();
    assert_eq!(fixture.payload(1), [1]);
    assert_eq!(fixture.payload(3), [3]);
    assert_eq!(fs::read(fixture.path("progress.norito")).unwrap(), [1, 3]);
}

#[test]
fn native_rewrite_creator_retry_after_data_publication_does_not_rebuild_or_rename_it() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(1, b"first", true).unwrap();
    transaction.append(2, b"second", true).unwrap();
    let policy = IndexedSidecarRewrite::Newest {
        retention: NonZeroUsize::new(1).unwrap(),
        pinned_height: None,
    };
    fail_next_sidecar_promotion_dir_sync_for_tests();
    assert!(transaction.rewrite(policy, "published creator").is_err());
    assert!(transaction.originals.data_published);
    assert!(!transaction.originals.index_published);
    let published =
        iroha_fs::FileIdentity::of(original(&transaction.originals.rewrite_data).unwrap()).unwrap();
    assert!(!fixture.path("progress.norito.tmp").exists());
    transaction.rewrite(policy, "published creator").unwrap();
    assert_eq!(
        iroha_fs::FileIdentity::of(original(&transaction.originals.data).unwrap()).unwrap(),
        published
    );
    assert_eq!(
        fs::read(fixture.path("progress.norito")).unwrap(),
        b"second"
    );
    assert_eq!(fixture.payload(2), b"second");
}

#[test]
fn native_rewrite_rejected_required_policy_leaves_no_creator_and_allows_valid_request() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(1, b"first", true).unwrap();
    transaction.append(2, b"second", true).unwrap();
    let required = BTreeSet::from([3]);
    assert!(
        transaction
            .rewrite(
                IndexedSidecarRewrite::AfterTerminalFrontier {
                    terminal_height: 2,
                    retention: NonZeroUsize::new(1).unwrap(),
                    required_heights: &required,
                },
                "missing required"
            )
            .is_err()
    );
    assert!(transaction.rewrite.is_none());
    assert!(transaction.originals.rewrite_data.is_none());
    assert!(transaction.originals.rewrite_index.is_none());
    transaction
        .rewrite(
            IndexedSidecarRewrite::Newest {
                retention: NonZeroUsize::new(1).unwrap(),
                pinned_height: None,
            },
            "valid after refusal",
        )
        .unwrap();
    assert_eq!(fixture.payload(2), b"second");
}

#[test]
fn native_rewrite_displaced_original_custody_survives_publication_error_until_close() {
    for step in [RecoveryStep::DataPublished, RecoveryStep::IndexPublished] {
        let fixture = Fixture::new();
        fixture.seed_pair("progress.norito", "progress.index", 1, b"old");
        fixture.seed_pair("progress.norito.tmp", "progress.index.tmp", 2, b"new");
        let mut transaction = fixture.capture().unwrap();
        let old = match step {
            RecoveryStep::DataPublished => original(&transaction.originals.data),
            RecoveryStep::IndexPublished => original(&transaction.originals.index),
        }
        .unwrap()
        .try_clone()
        .unwrap();
        let identity = iroha_fs::FileIdentity::of(&old).unwrap();
        let initial_permissions = old.metadata().unwrap().permissions();
        let failure = transaction
            .recover_observed(&BTreeSet::new(), &mut |observed| {
                if observed == step {
                    let mut changed = initial_permissions.clone();
                    changed.set_readonly(true);
                    old.set_permissions(changed)?;
                    return Err(io::Error::other("first post-publication failure"));
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(failure.to_string(), "first post-publication failure");
        assert!(old.metadata().unwrap().permissions().readonly());
        assert!(transaction.verify().is_err());
        assert!(
            transaction.recover().is_err(),
            "retry must not close a refused displaced original"
        );
        let still_held = match step {
            RecoveryStep::DataPublished => original(&transaction.originals.data),
            RecoveryStep::IndexPublished => original(&transaction.originals.index),
        }
        .unwrap();
        assert_eq!(iroha_fs::FileIdentity::of(still_held).unwrap(), identity);
        old.set_permissions(initial_permissions).unwrap();
        transaction.recover().unwrap();
        assert_eq!(fixture.payload(2), b"new");
        assert!(transaction.originals.displaced_data_identity.is_none());
        assert!(transaction.originals.displaced_index_identity.is_none());
    }
}

#[test]
fn native_prepend_displaced_index_is_revalidated_before_retry_close() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture().unwrap();
    transaction.append(3, b"three", true).unwrap();
    let old = original(&transaction.originals.index)
        .unwrap()
        .try_clone()
        .unwrap();
    let identity = iroha_fs::FileIdentity::of(&old).unwrap();
    let initial_permissions = old.metadata().unwrap().permissions();
    fail_next_indexed_sidecar_dir_sync_for_tests();
    assert!(transaction.append(1, b"one", true).is_err());
    assert!(transaction.pending.as_ref().unwrap().published);
    let mut changed = initial_permissions.clone();
    changed.set_readonly(true);
    old.set_permissions(changed).unwrap();
    assert!(old.metadata().unwrap().permissions().readonly());
    assert!(transaction.recover().is_err());
    assert_eq!(
        iroha_fs::FileIdentity::of(original(&transaction.originals.index).unwrap()).unwrap(),
        identity
    );
    assert_eq!(
        transaction.originals.displaced_index_identity,
        Some(identity)
    );
    old.set_permissions(initial_permissions).unwrap();
    transaction.recover().unwrap();
    assert_eq!(fixture.payload(1), b"one");
    assert_eq!(fixture.payload(3), b"three");
    assert!(transaction.originals.displaced_index_identity.is_none());
}

#[test]
fn native_pipeline_directory_uses_only_canonical_block_parent() {
    let kura = Kura::blank_kura_for_testing();
    let canonical = Kura::canonical_storage_path(&kura.store_root);
    assert_eq!(kura.store_dir().unwrap(), canonical);
    let expected = canonical.join(PIPELINE_DIR_NAME);
    assert!(!expected.exists());
    let child = kura.ensure_native_pipeline_directory(&expected).unwrap();
    assert_eq!(child.path(), expected);
    let identity = child.identity().unwrap();
    child.revalidate().unwrap();
    // The newly created child has actual private custody for staged test inputs.
    drop(iroha_fs::PrivateDirectory::open(&expected).unwrap());
    assert!(!kura.store_root.join(PIPELINE_DIR_NAME).exists());
    drop(child);
    assert_eq!(
        kura.ensure_native_pipeline_directory(&expected)
            .unwrap()
            .identity()
            .unwrap(),
        identity
    );
}

#[test]
fn native_pipeline_directory_refuses_root_level_or_other_expected_path() {
    let kura = Kura::blank_kura_for_testing();
    let canonical = Kura::canonical_storage_path(&kura.store_root);
    for wrong in [
        kura.store_root.join(PIPELINE_DIR_NAME),
        canonical.join("other-pipeline"),
        kura.store_root.join("blocks/other").join(PIPELINE_DIR_NAME),
    ] {
        let error = kura.ensure_native_pipeline_directory(&wrong).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(!wrong.exists());
    }
    assert!(!canonical.join(PIPELINE_DIR_NAME).exists());
}

#[test]
fn native_pipeline_directory_does_not_create_missing_or_replace_nondirectory_parents() {
    for component in ["blocks", "canonical"] {
        let kura = Kura::blank_kura_for_testing();
        let canonical = Kura::canonical_storage_path(&kura.store_root);
        let expected = canonical.join(PIPELINE_DIR_NAME);
        let marker = fs::read(canonical.join(COUNT_FILE_NAME)).unwrap();
        kura.block_store.lock().drop_cached_handles();
        let missing = if component == "blocks" {
            kura.store_root.join("blocks")
        } else {
            canonical.clone()
        };
        let displaced = missing.with_file_name(format!("held-{component}"));
        fs::rename(&missing, &displaced).unwrap();
        let error = kura
            .ensure_native_pipeline_directory(&expected)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(!missing.exists());
        assert!(!expected.exists());
        fs::write(&missing, b"not a directory").unwrap();
        assert!(kura.ensure_native_pipeline_directory(&expected).is_err());
        assert_eq!(fs::read(&missing).unwrap(), b"not a directory");
        fs::remove_file(&missing).unwrap();
        fs::rename(&displaced, &missing).unwrap();
        assert_eq!(fs::read(canonical.join(COUNT_FILE_NAME)).unwrap(), marker);
        assert!(!expected.exists());
        kura.ensure_native_pipeline_directory(&expected)
            .unwrap()
            .revalidate()
            .unwrap();
    }
}

#[test]
fn native_pipeline_returned_child_retains_ancestors_and_refuses_late_acl_drift() {
    let kura = Kura::blank_kura_for_testing();
    let canonical = Kura::canonical_storage_path(&kura.store_root);
    let expected = canonical.join(PIPELINE_DIR_NAME);
    let child = kura.ensure_native_pipeline_directory(&expected).unwrap();
    let before = child.identity().unwrap();
    // Cached block files cannot be the reason this ancestor rename is excluded.
    kura.block_store.lock().drop_cached_handles();
    let displaced = canonical.with_file_name("replaced-canonical");
    assert!(fs::rename(&canonical, &displaced).is_err());
    assert!(!displaced.exists());
    assert_eq!(child.identity().unwrap(), before);
    child.revalidate().unwrap();
    let status = std::process::Command::new("icacls")
        .arg(&canonical)
        .args(["/grant", "*S-1-1-0:(W)"])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(child.revalidate().is_err());
    assert!(kura.ensure_native_pipeline_directory(&expected).is_err());
    assert!(expected.is_dir());
    assert!(!kura.store_root.join(PIPELINE_DIR_NAME).exists());
}
