//! Native journal invariants shared by Unix and Windows; no finalized authority is synthesized.
use super::*;

fn directory() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::Builder::new()
        .prefix(".portable-signer-journal-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    let private = PrivateDirectory::open_or_create(temporary.path().join("receipts")).unwrap();
    let path = private.path().to_owned();
    drop(private);
    (temporary, path)
}

#[test]
fn native_path_preflight_bounds_exact_absolute_spelling_before_filesystem_access() {
    let root = if cfg!(windows) {
        Path::new("C:\\")
    } else {
        Path::new("/")
    };
    let profile = JournalProfile::receipt(SignerReceiptPurposeV1::StreamToken);
    let mut maximum = root.to_owned();
    for _ in 0..MAX_JOURNAL_PATH_COMPONENTS {
        maximum.push("a");
    }
    assert_eq!(
        preflight_journal_path(&maximum, profile).unwrap().len(),
        MAX_JOURNAL_PATH_COMPONENTS
    );
    assert!(preflight_journal_path(&maximum.join("a"), profile).is_err());
    let prefix_bytes = root.as_os_str().as_encoded_bytes().len();
    let exact_bytes = root.join("a".repeat(MAX_JOURNAL_PATH_BYTES - prefix_bytes));
    assert_eq!(
        exact_bytes.as_os_str().as_encoded_bytes().len(),
        MAX_JOURNAL_PATH_BYTES
    );
    preflight_journal_path(&exact_bytes, profile).unwrap();
    let mut excessive = exact_bytes.into_os_string();
    excessive.push("a");
    assert!(preflight_journal_path(Path::new(&excessive), profile).is_err());
    for invalid in [
        PathBuf::from("relative/journal"),
        root.to_owned(),
        root.join("a/../journal"),
        root.join("a/./journal"),
        root.join("a//journal"),
    ] {
        assert!(preflight_journal_path(&invalid, profile).is_err());
    }
}

#[test]
fn native_read_only_receipt_recovers_exact_bytes_and_retains_the_original_lease() {
    let (_temporary, path) = directory();
    let operation = [0x51; 32];
    let journal =
        SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).unwrap();
    let reader = journal.reader();
    let pinned = journal.stage(operation, b"exact native receipt").unwrap();
    let name = format!("{}{SUFFIX}", hex::encode(operation));
    let directory = PrivateDirectory::open_exact(&path).unwrap();
    directory.open_retained_read_only(&name, 64).unwrap();
    assert_eq!(journal.reader.inner.inventory().unwrap(), (1, 20));
    assert!(journal.ensure_unstaged(operation).is_err());
    assert!(journal.stage(operation, b"replacement").is_err());
    drop(journal);
    assert!(SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).is_err());
    assert_eq!(
        reader.recover(operation).unwrap().bytes(),
        b"exact native receipt"
    );
    drop(reader);
    assert!(SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).is_err());
    pinned.recheck().unwrap();
    drop(pinned);
    drop(directory);
    let reopened =
        SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).unwrap();
    assert_eq!(
        reopened.recover(operation).unwrap().bytes(),
        b"exact native receipt"
    );
}

#[test]
fn native_receipt_recovery_rejects_owner_writable_records_and_unrecognized_entries() {
    let (_temporary, path) = directory();
    let operation = [0x52; 32];
    let name = format!("{}{SUFFIX}", hex::encode(operation));
    let private = PrivateDirectory::open_exact(&path).unwrap();
    private
        .write_atomic(&name, b"unsealed", iroha_fs::PublishMode::CreateNew)
        .unwrap();
    assert!(SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).is_err());
    let (_other, other_path) = directory();
    let journal =
        SignerReceiptJournalV1::open_test(&other_path, SignerReceiptPurposeV1::StreamToken)
            .unwrap();
    PrivateDirectory::open_exact(&other_path)
        .unwrap()
        .write_atomic("unknown", b"x", iroha_fs::PublishMode::CreateNew)
        .unwrap();
    assert!(journal.ensure_unstaged(operation).is_err());
    assert!(
        journal
            .stage(operation, b"no admission through corrupt inventory")
            .is_err()
    );
}

#[test]
fn native_inventory_counts_exact_data_limits_without_counting_its_control_lock() {
    let (_temporary, path) = directory();
    let mut profile = JournalProfile::receipt(SignerReceiptPurposeV1::StreamToken);
    profile.max_records = 2;
    profile.max_total_bytes = 6;
    profile.max_record_bytes = 3;
    let inner = JournalInner::open_test(&path, profile).unwrap();
    assert_eq!(inner.inventory().unwrap(), (0, 0));
    assert!(inner.stage([0; 32], b"abc").is_err());
    assert!(inner.stage([1; 32], b"abcd").is_err());
    assert!(inner.stage([1; 32], b"").is_err());
    assert_eq!(inner.inventory().unwrap(), (0, 0));
    let first = inner.stage([1; 32], b"abc").unwrap();
    let second = inner.stage([2; 32], b"def").unwrap();
    assert_eq!(inner.inventory().unwrap(), (2, 6));
    assert!(inner.stage([3; 32], b"g").is_err());
    assert_eq!(first.bytes(), b"abc");
    assert_eq!(second.bytes(), b"def");
    assert_eq!(
        PrivateDirectory::open_exact(&path)
            .unwrap()
            .entries(3)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn native_incomplete_final_receipt_is_a_durable_refusal_after_restart() {
    let (_temporary, path) = directory();
    let operation = [0x53; 32];
    let journal =
        SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).unwrap();
    let private = PrivateDirectory::open_exact(&path).unwrap();
    let mut interrupted = private
        .create_retained_private(format!("{}{SUFFIX}", hex::encode(operation)), 64)
        .unwrap();
    interrupted.write_all(b"partial").unwrap();
    drop(interrupted);
    assert!(journal.ensure_unstaged(operation).is_err());
    assert!(journal.recover(operation).is_err());
    drop(journal);
    assert!(SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).is_err());
}

#[test]
fn native_pending_reserve_preserves_tombstones_and_no_replace_publication() {
    let (temporary, _) = directory();
    let pending =
        PrivateDirectory::open_or_create(temporary.path().join(PENDING_RESERVE_DIRECTORY)).unwrap();
    let path = pending.path().to_owned();
    let files = SignerPendingReserveFilesV1::open_test(&path).unwrap();
    let first = files.stage([1; 32], b"first exact signed reserve").unwrap();
    let interrupted = [2; 32];
    assert!(
        files
            .inner
            .stage_pending_reserve_with(interrupted, b"second exact signed reserve", |point| {
                if point == PendingReserveCheckpoint::SignedBytesDurable {
                    Err(io::Error::other(
                        "synthetic interruption after durable signed bytes",
                    ))
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    assert!(files.recover(interrupted).is_err());
    assert!(files.stage(interrupted, b"replacement").is_err());
    first.recheck().unwrap();
    drop(first);
    drop(files);
    let reopened = SignerPendingReserveFilesV1::open_test(&path).unwrap();
    assert_eq!(
        reopened.recover([1; 32]).unwrap().bytes(),
        b"first exact signed reserve"
    );
    assert!(reopened.recover(interrupted).is_err());
    assert!(reopened.stage(interrupted, b"replacement").is_err());
    assert_eq!(
        reopened
            .stage([3; 32], b"later exact signed reserve")
            .unwrap()
            .bytes(),
        b"later exact signed reserve"
    );
}

#[test]
fn native_lock_control_substitution_never_becomes_missing_receipt_authority() {
    let (_temporary, path) = directory();
    let journal =
        SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).unwrap();
    let control = path.join(LOCK_NAME);
    let rename = std::fs::rename(&control, path.join("replaced-control"));
    #[cfg(unix)]
    {
        rename.expect("Unix permits renaming the retained locked inode in this fixture");
        // The exact retained identity must still fence the substituted name.
        PrivateDirectory::open_exact(&path)
            .unwrap()
            .write_atomic(LOCK_NAME, b"", iroha_fs::PublishMode::CreateNew)
            .unwrap();
        assert!(journal.ensure_unstaged([4; 32]).is_err());
        assert!(journal.stage([4; 32], b"must remain refused").is_err());
    }
    #[cfg(windows)]
    {
        // MoveFileExW refuses this retained no-delete-sharing file with ERROR_ACCESS_DENIED (5)
        // or ERROR_SHARING_VIOLATION (32); unrelated fixture errors cannot satisfy the assertion.
        let error = rename.expect_err("Windows must refuse the retained control-file substitution");
        assert!(
            matches!(error.raw_os_error(), Some(5 | 32)),
            "unexpected rename refusal: {error}"
        );
        journal.ensure_unstaged([4; 32]).unwrap();
        assert!(
            SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).is_err()
        );
    }
}

#[test]
fn native_pinned_receipt_excludes_a_separate_process_until_its_last_owner_drops() {
    const CHILD_PATH: &str = "IROHA_PORTABLE_SIGNER_JOURNAL_LEASE_CHILD_PATH";
    const CHILD_TEST: &str = "signer_operation::journal::portable_tests::native_pinned_receipt_excludes_a_separate_process_until_its_last_owner_drops";
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        assert!(
            SignerReceiptJournalV1::open_test(
                Path::new(&path),
                SignerReceiptPurposeV1::StreamToken
            )
            .is_err()
        );
        return;
    }
    let (_temporary, path) = directory();
    let writer =
        SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).unwrap();
    let pinned = writer.stage([7; 32], b"retained process lease").unwrap();
    drop(writer);
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", CHILD_TEST, "--nocapture", "--color", "never"])
        .env(CHILD_PATH, &path)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "the exact child lease probe must succeed"
    );
    let stdout = std::str::from_utf8(&child.stdout).unwrap();
    for required in [
        "running 1 test".to_owned(),
        format!("test {CHILD_TEST} ... ok"),
    ] {
        assert_eq!(
            stdout.lines().filter(|line| *line == required).count(),
            1,
            "the exact child lease probe must run once"
        );
    }
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line
                .starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "))
            .count(),
        1
    );
    pinned.recheck().unwrap();
    drop(pinned);
    let reopened =
        SignerReceiptJournalV1::open_test(&path, SignerReceiptPurposeV1::StreamToken).unwrap();
    assert_eq!(
        reopened.recover([7; 32]).unwrap().bytes(),
        b"retained process lease"
    );
}

#[test]
fn native_pending_publication_never_replaces_a_competing_final_identity() {
    let (temporary, _) = directory();
    let directory =
        PrivateDirectory::open_or_create(temporary.path().join(PENDING_RESERVE_DIRECTORY)).unwrap();
    let files = SignerPendingReserveFilesV1::open_test(directory.path()).unwrap();
    let operation = [8; 32];
    let final_name = format!("{}{PENDING_RESERVE_SUFFIX}", hex::encode(operation));
    let mut competing = None;
    assert!(
        files
            .inner
            .stage_pending_reserve_with(operation, b"original pending bytes", |point| {
                if point == PendingReserveCheckpoint::SignedBytesDurable {
                    let mut writer = directory.create_retained_private(&final_name, 64)?;
                    writer.write_all(b"competing final identity")?;
                    competing = Some(writer.seal_read_only()?);
                }
                Ok(())
            })
            .is_err()
    );
    let mut competing =
        competing.expect("collision was injected after original bytes were durable");
    let mut bytes = Vec::new();
    competing.seek(SeekFrom::Start(0)).unwrap();
    competing.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"competing final identity");
    let inflight_name = format!(
        "{}{PENDING_RESERVE_IN_PROGRESS_SUFFIX}",
        hex::encode(operation)
    );
    let mut inflight = directory
        .open_retained_read_only(&inflight_name, 64)
        .unwrap();
    bytes.clear();
    inflight.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original pending bytes");
    assert!(files.stage(operation, b"replacement").is_err());
    drop(inflight);
    drop(competing);
    drop(files);
    assert!(
        SignerPendingReserveFilesV1::open_test(directory.path()).is_err(),
        "two identities for one operation remain a closed ambiguity after restart"
    );
}
