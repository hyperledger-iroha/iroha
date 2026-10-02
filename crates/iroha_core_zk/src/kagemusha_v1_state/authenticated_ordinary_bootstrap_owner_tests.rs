//! Actual exclusive storage creation and cold replay under an existing Native enrollment root.
//!
//! These storage regressions create no financial, State, Guard or publication authority.

use super::*;

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "storage-layout-test.wal",
    magic: b"IKGBST1\0",
    hash_domain: b"iroha:kagemusha:v1:test:bootstrap-storage-layout\0",
    maximum_payload_bytes: 1024,
};

#[test]
fn bootstrap_purpose_journals_create_and_cold_recover_under_retained_enrollment_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp
        .path()
        .canonicalize()
        .unwrap()
        .join("native-enrollment");
    // C21 already owns this directory before Bootstrap begins. Reusing it as an exclusive
    // logical/platform/current journal path used to fail before any platform approval.
    let mut enrollment = PrivateJournal::create_new(&root, FORMAT).unwrap();
    enrollment.append(b"retained C21 original").unwrap();
    let enrollment_prefix = enrollment.recovery_prefix().unwrap();
    let purposes = [
        BootstrapStoragePurpose::LogicalApproval,
        BootstrapStoragePurpose::PlatformAttempt,
        BootstrapStoragePurpose::CurrentPublication,
    ];
    let originals = [
        b"logical original".as_slice(),
        b"platform original".as_slice(),
        b"current original".as_slice(),
    ];
    let paths = purposes.map(|purpose| bootstrap_storage_path(&root, purpose));
    let journals: Vec<_> = paths
        .iter()
        .zip(originals)
        .map(|(path, original)| {
            let mut journal = PrivateJournal::create_new(path, FORMAT).unwrap();
            journal.append(original).unwrap();
            journal
        })
        .collect();
    let prefixes: Vec<_> = journals
        .iter()
        .map(|journal| journal.recovery_prefix().unwrap())
        .collect();
    let bytes: Vec<_> = paths
        .iter()
        .map(|path| std::fs::read(path.join(FORMAT.filename)).unwrap())
        .collect();
    for (journal, original) in journals.iter().zip(originals) {
        journal.check_owned().unwrap();
        journal.require_single_record(original).unwrap();
    }
    assert_eq!(enrollment.recovery_prefix().unwrap(), enrollment_prefix);
    for path in &paths {
        assert!(PrivateJournal::create_new(path, FORMAT).is_err());
    }
    drop(journals);

    for (((path, original), prefix), bytes) in paths.iter().zip(originals).zip(prefixes).zip(bytes)
    {
        let mut reopened = PrivateJournal::open_existing(path, FORMAT).unwrap();
        reopened.check_owned().unwrap();
        assert_eq!(
            reopened.replay_next().unwrap(),
            Some((0, original.to_vec()))
        );
        assert_eq!(reopened.replay_next().unwrap(), None);
        assert_eq!(reopened.recovery_prefix().unwrap(), prefix);
        assert_eq!(std::fs::read(path.join(FORMAT.filename)).unwrap(), bytes);
    }
    assert_eq!(enrollment.recovery_prefix().unwrap(), enrollment_prefix);
    assert!(PrivateJournal::create_new(&root, FORMAT).is_err());
}
