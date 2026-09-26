//! Read-only capabilities retain the original lease and exact bounded pinned identities.
use super::*;

const PURPOSE: SignerReceiptPurposeV1 = SignerReceiptPurposeV1::FinalPromotionProvenance;
const OPERATION: [u8; 32] = [0x29; 32];

fn record_path(path: &Path) -> std::path::PathBuf {
    path.join(format!("{}{SUFFIX}", hex::encode(OPERATION)))
}

#[test]
fn reader_and_pinned_receipt_keep_the_same_exclusive_lease() {
    const CHILD_PATH: &str = "IROHA_SIGNER_RECEIPT_READER_LEASE_TEST_CHILD_PATH";
    const CHILD_TEST: &str = "signer_operation::journal::tests::reader::reader_and_pinned_receipt_keep_the_same_exclusive_lease";
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        assert!(SignerReceiptJournalV1::open_test(Path::new(&path), PURPOSE).is_err());
        return;
    }
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let writer = SignerReceiptJournalV1::open_test(&path, PURPOSE).unwrap();
    let reader = writer.reader();
    assert_eq!(reader.purpose(), PURPOSE);
    assert!(reader.recover(OPERATION).is_err());
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    let bytes = b"exact private receipt bytes";
    drop(writer.stage(OPERATION, bytes).unwrap());
    drop(writer);
    assert!(SignerReceiptJournalV1::open_test(&path, PURPOSE).is_err());
    let pinned = reader.recover(OPERATION).unwrap();
    assert_eq!(pinned.bytes(), bytes);
    drop(reader);
    for purpose in [
        PURPOSE,
        SignerReceiptPurposeV1::ReleaseManifest,
        SignerReceiptPurposeV1::StreamToken,
    ] {
        assert!(SignerReceiptJournalV1::open_test(&path, purpose).is_err());
    }
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(CHILD_TEST)
        .arg("--nocapture")
        .args(["--color", "never"])
        .env(CHILD_PATH, &path)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "pinned reader must exclude another process"
    );
    let child_output = std::str::from_utf8(&child.stdout).unwrap();
    for required in [
        "running 1 test".to_owned(),
        format!("test {CHILD_TEST} ... ok"),
    ] {
        assert_eq!(
            child_output
                .lines()
                .filter(|line| *line == required)
                .count(),
            1,
            "the exact child lease probe must actually run once"
        );
    }
    assert_eq!(
        child_output
            .lines()
            .filter(|line| line
                .starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "))
            .count(),
        1,
        "a missing or ignored child probe must not satisfy lease exclusion"
    );
    pinned.recheck().unwrap();
    assert_eq!(pinned.bytes(), bytes);
    drop(pinned);
    let reopened = SignerReceiptJournalV1::open_test(&path, PURPOSE)
        .expect("the last pinned owner releases the lease");
    assert_eq!(reopened.recover(OPERATION).unwrap().bytes(), bytes);
}

#[test]
fn staged_receipt_also_retains_the_lease_after_the_writer_drops() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let writer = SignerReceiptJournalV1::open_test(&path, PURPOSE).unwrap();
    let staged = writer.stage(OPERATION, b"staged exact bytes").unwrap();
    drop(writer);
    assert!(SignerReceiptJournalV1::open_test(&path, PURPOSE).is_err());
    staged.recheck().unwrap();
    assert_eq!(staged.bytes(), b"staged exact bytes");
    drop(staged);
    SignerReceiptJournalV1::open_test(&path, PURPOSE).unwrap();
}

#[test]
fn read_only_recovery_has_the_same_closed_purpose_bounds() {
    for purpose in [
        PURPOSE,
        SignerReceiptPurposeV1::ReleaseManifest,
        SignerReceiptPurposeV1::StreamToken,
    ] {
        let directory = private_directory();
        let path = directory.path().canonicalize().unwrap();
        let writer = SignerReceiptJournalV1::open_test(&path, purpose).unwrap();
        let reader = writer.reader();
        assert_eq!(reader.purpose(), purpose);
        assert!(reader.recover([0; 32]).is_err());
        assert!(reader.recover(OPERATION).is_err());
        assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
        let bytes = vec![0x43; purpose.max_bytes()];
        drop(writer.stage(OPERATION, &bytes).unwrap());
        let pinned = reader.recover(OPERATION).unwrap();
        assert_eq!(pinned.bytes(), bytes);
        pinned.recheck().unwrap();
        let file = record_path(&path);
        fs::set_permissions(&file, Permissions::from_mode(0o600)).unwrap();
        fs::write(&file, vec![0x43; purpose.max_bytes() + 1]).unwrap();
        fs::set_permissions(&file, Permissions::from_mode(0o400)).unwrap();
        assert!(pinned.recheck().is_err());
        assert!(reader.recover(OPERATION).is_err());
        assert_eq!(
            pinned.bytes(),
            bytes,
            "a pinned buffer must not be replaced"
        );
    }
}

#[test]
fn reader_recheck_rejects_in_place_byte_changes_and_retains_its_original_buffer() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let writer = SignerReceiptJournalV1::open_test(&path, PURPOSE).unwrap();
    drop(writer.stage(OPERATION, b"original bytes").unwrap());
    let reader = writer.reader();
    let pinned = reader.recover(OPERATION).unwrap();
    let file = record_path(&path);
    fs::set_permissions(&file, Permissions::from_mode(0o600)).unwrap();
    fs::write(&file, b"replaced bytes").unwrap();
    fs::set_permissions(&file, Permissions::from_mode(0o400)).unwrap();
    assert!(pinned.recheck().is_err());
    assert_eq!(pinned.bytes(), b"original bytes");
}

#[test]
fn reader_recheck_rejects_path_replacement_even_when_the_bytes_match() {
    let directory = private_directory();
    let path = directory.path().canonicalize().unwrap();
    let writer = SignerReceiptJournalV1::open_test(&path, PURPOSE).unwrap();
    let bytes = b"identical bytes do not preserve the pinned inode";
    drop(writer.stage(OPERATION, bytes).unwrap());
    let reader = writer.reader();
    let pinned = reader.recover(OPERATION).unwrap();
    let file = record_path(&path);
    fs::rename(&file, path.join("retained-original")).unwrap();
    fs::write(&file, bytes).unwrap();
    fs::set_permissions(&file, Permissions::from_mode(0o400)).unwrap();
    assert!(pinned.recheck().is_err());
    assert_eq!(pinned.bytes(), bytes);
}

#[test]
fn read_only_recovery_rejects_unsafe_files_and_changed_ancestor_identity() {
    for attack in ["hardlink", "symlink", "writable", "ancestor"] {
        let directory = private_directory();
        let parent = directory.path().canonicalize().unwrap();
        let path = parent.join("receipts");
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
        let writer = SignerReceiptJournalV1::open_test(&path, PURPOSE).unwrap();
        drop(writer.stage(OPERATION, b"private receipt").unwrap());
        let reader = writer.reader();
        let pinned = reader.recover(OPERATION).unwrap();
        let file = record_path(&path);
        match attack {
            "hardlink" => fs::hard_link(&file, parent.join("second-link")).unwrap(),
            "symlink" => {
                let retained = parent.join("retained-record");
                fs::rename(&file, &retained).unwrap();
                std::os::unix::fs::symlink(retained, &file).unwrap();
            }
            "writable" => {
                fs::set_permissions(&file, Permissions::from_mode(0o600)).unwrap();
            }
            "ancestor" => {
                fs::rename(&path, parent.join("retained-directory")).unwrap();
                fs::create_dir(&path).unwrap();
                fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
                fs::write(&file, b"private receipt").unwrap();
                fs::set_permissions(&file, Permissions::from_mode(0o400)).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(pinned.recheck().is_err(), "{attack}");
        assert!(reader.recover(OPERATION).is_err(), "{attack}");
        assert_eq!(pinned.bytes(), b"private receipt", "{attack}");
    }
}
