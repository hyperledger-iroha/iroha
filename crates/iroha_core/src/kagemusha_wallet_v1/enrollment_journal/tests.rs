//! Native filesystem ordering tests. Opaque DATA does not stand in for platform evidence.
use super::*;

impl EnrollmentJournalV1 {
    // Ordering tests use opaque DATA. This helper cannot enter the production dispatch path.
    fn select_verification_originals(
        &mut self,
        attempt: &mut EnrollmentAttemptV1,
        request: Vec<u8>,
        worker_request: Vec<u8>,
        verification_time_ms: u64,
    ) -> Result<EnrollmentVerificationDispatchV1> {
        self.select_verification_bound(
            attempt,
            request,
            worker_request,
            verification_time_ms,
            [9; 32],
        )
    }
}

fn selection() -> EnrollmentSelectionV1 {
    EnrollmentSelectionV1 {
        key: [1; 32],
        attempt_id: [2; 32],
        challenge: KagemushaWalletEnrollmentChallengeV1 {
            version: 1,
            scheme_id: [3; 32],
            asset_digest: [4; 32],
            account_digest: [5; 32],
            app_policy: [6; 32],
            enrollment_policy: [7; 32],
            issuer_nonce: [8; 32],
        },
        created_at_ms: 1_000,
        expires_at_ms: 601_001,
        stable_selection: b"unadmitted stable scope DATA".to_vec(),
    }
}
pub(super) fn initialized() -> (tempfile::TempDir, PrivateDirectory, EnrollmentJournalV1) {
    let temp = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let parent = PrivateDirectory::open(temp.path()).unwrap();
    let journal =
        EnrollmentJournalV1::initialize(&parent, "issuer", b"approved scope DATA").unwrap();
    (temp, parent, journal)
}

#[test]
fn request_indexes_bind_issuer_scope_subject_and_original_request() {
    let (temp, parent, journal) = initialized();
    let first = journal.request_key(&[1; 32], &[2; 32]).unwrap();
    assert_eq!(first, journal.request_key(&[1; 32], &[2; 32]).unwrap());
    assert_ne!(first, journal.request_key(&[2; 32], &[1; 32]).unwrap());
    assert_ne!(first, journal.request_key(&[1; 32], &[3; 32]).unwrap());
    assert_eq!(journal.request_key(&[0; 32], &[2; 32]), Err(Invalid));
    assert_eq!(journal.request_key(&[1; 32], &[0; 32]), Err(Invalid));
    let other =
        EnrollmentJournalV1::initialize(&parent, "other", b"different issuer DATA").unwrap();
    assert_ne!(first, other.request_key(&[1; 32], &[2; 32]).unwrap());
    drop(journal);
    let journal =
        EnrollmentJournalV1::open(&temp.path().join("issuer"), b"approved scope DATA").unwrap();
    assert_eq!(first, journal.request_key(&[1; 32], &[2; 32]).unwrap());
}

#[test]
fn original_selection_deadline_and_exact_result_survive_restart() {
    let (temp, _parent, mut journal) = initialized();
    let mut attempt = journal.select(selection()).unwrap();
    assert_eq!(attempt.phase(), EnrollmentJournalPhaseV1::Selected);
    assert!(attempt.verification().is_none());
    let dispatch = journal
        .select_verification_originals(
            &mut attempt,
            b"E5 DATA".to_vec(),
            b"worker request DATA".to_vec(),
            601_000,
        )
        .unwrap();
    assert_eq!(dispatch.into_original(), b"worker request DATA");
    journal
        .retain_worker_result(&mut attempt, b"worker result DATA".to_vec(), false)
        .unwrap();
    let mut signing = attempt.record.clone();
    signing.phase = EnrollmentJournalPhaseV1::Signing;
    signing.credential_body = b"credential body DATA".to_vec();
    journal.advance(&mut attempt, signing).unwrap();
    journal
        .retain_issued(&mut attempt, b"signed E6 DATA".to_vec())
        .unwrap();
    drop(journal);
    let mut journal =
        EnrollmentJournalV1::open(&temp.path().join("issuer"), b"approved scope DATA").unwrap();
    let mut restored = journal.select(selection()).unwrap();
    assert_eq!(restored.selection(), &selection());
    assert_eq!(restored.issued(), Some(b"signed E6 DATA".as_slice()));
    assert_eq!(
        restored.worker_result(),
        Some(b"worker result DATA".as_slice())
    );
    assert_eq!(
        restored.verification(),
        Some((
            b"E5 DATA".as_slice(),
            b"worker request DATA".as_slice(),
            601_000
        ))
    );
    journal
        .retain_issued(&mut restored, b"signed E6 DATA".to_vec())
        .unwrap();
    assert_eq!(
        journal.retain_issued(&mut restored, b"different signature".to_vec()),
        Err(Conflict)
    );
    let mut changed = selection();
    changed.expires_at_ms += 1;
    assert!(matches!(journal.select(changed), Err(Conflict)));
}

#[test]
fn a_restart_or_second_cursor_cannot_repeat_a_verification() {
    let (temp, _parent, mut journal) = initialized();
    let mut attempt = journal.select(selection()).unwrap();
    let mut stale = journal.read(&selection().key).unwrap().unwrap();
    let _dispatch = journal
        .select_verification_originals(&mut attempt, vec![1], vec![2], 1_000)
        .unwrap();
    assert!(matches!(
        journal.select_verification_originals(&mut stale, vec![1], vec![2], 1_000),
        Err(Conflict)
    ));
    drop(journal);
    let mut journal =
        EnrollmentJournalV1::open(&temp.path().join("issuer"), b"approved scope DATA").unwrap();
    let mut restored = journal.read(&selection().key).unwrap().unwrap();
    assert_eq!(restored.phase(), EnrollmentJournalPhaseV1::Verifying);
    assert!(matches!(
        journal.select_verification_originals(&mut restored, vec![1], vec![2], 1_000),
        Err(Conflict)
    ));
    // Absence of a retained worker result is outcome-unknown, not permission to Verify.
    assert!(restored.worker_result().is_none());
    assert_eq!(journal.retain_issued(&mut restored, vec![3]), Err(Conflict));
}

#[test]
fn rejection_is_terminal_and_mismatched_result_cannot_replace_it() {
    let (_temp, _parent, mut journal) = initialized();
    let mut attempt = journal.select(selection()).unwrap();
    let _ = journal
        .select_verification_originals(&mut attempt, vec![1], vec![2], 2_000)
        .unwrap();
    journal
        .retain_worker_result(&mut attempt, vec![3], true)
        .unwrap();
    journal
        .retain_worker_result(&mut attempt, vec![3], true)
        .unwrap();
    assert_eq!(attempt.phase(), EnrollmentJournalPhaseV1::Rejected);
    assert_eq!(
        journal.retain_worker_result(&mut attempt, vec![3], false),
        Err(Conflict)
    );
    assert_eq!(
        journal.retain_worker_result(&mut attempt, vec![4], true),
        Err(Conflict)
    );
    assert_eq!(journal.retain_issued(&mut attempt, vec![5]), Err(Conflict));
}

#[test]
fn challenge_interval_and_all_original_bounds_fail_before_dispatch() {
    let (_temp, _parent, mut journal) = initialized();
    let mut attempt = journal.select(selection()).unwrap();
    for now in [0, 999, 601_001, u64::MAX] {
        assert!(matches!(
            journal.select_verification_originals(&mut attempt, vec![1], vec![2], now),
            Err(Invalid)
        ));
        assert_eq!(attempt.phase(), EnrollmentJournalPhaseV1::Selected);
    }
    for (request, worker) in [
        (vec![], vec![2]),
        (vec![1], vec![]),
        (vec![1; ORIGINAL_MAX + 1], vec![2]),
    ] {
        assert!(matches!(
            journal.select_verification_originals(&mut attempt, request, worker, 1_000),
            Err(Invalid)
        ));
    }
    let _ = journal
        .select_verification_originals(&mut attempt, vec![1], vec![2], 1_000)
        .unwrap();
    assert_eq!(
        journal.retain_worker_result(&mut attempt, Vec::new(), false),
        Err(Invalid)
    );
    journal
        .retain_worker_result(&mut attempt, vec![3], false)
        .unwrap();
    let mut signing = attempt.record.clone();
    signing.phase = EnrollmentJournalPhaseV1::Signing;
    signing.credential_body = vec![4];
    journal.advance(&mut attempt, signing).unwrap();
    assert_eq!(
        journal.retain_issued(&mut attempt, Vec::new()),
        Err(Invalid)
    );
    let mut invalid = selection();
    invalid.key = [0; 32];
    assert!(matches!(journal.select(invalid), Err(Invalid)));
    assert!(matches!(journal.read(&[0; 32]), Err(Invalid)));
}

#[test]
fn custody_is_exclusive_and_reopen_never_reinitializes_missing_files() {
    let (temp, parent, journal) = initialized();
    assert!(
        EnrollmentJournalV1::open(&temp.path().join("issuer"), b"approved scope DATA").is_err()
    );
    assert!(EnrollmentJournalV1::initialize(&parent, "issuer", b"approved scope DATA").is_err());
    drop(journal);
    assert!(EnrollmentJournalV1::open(&temp.path().join("issuer"), b"foreign scope DATA").is_err());
    std::fs::remove_file(temp.path().join("issuer").join(SCOPE)).unwrap();
    assert!(
        EnrollmentJournalV1::open(&temp.path().join("issuer"), b"approved scope DATA").is_err()
    );
    assert!(!temp.path().join("issuer").join(SCOPE).exists());
    assert!(
        EnrollmentJournalV1::open(&temp.path().join("missing"), b"approved scope DATA").is_err()
    );
    assert!(!temp.path().join("missing").exists());
}

#[test]
fn corrupt_or_foreign_records_are_not_absent_and_stale_mutations_fail() {
    let (_temp, _parent, mut journal) = initialized();
    assert!(journal.read(&[9; 32]).unwrap().is_none());
    let mut attempt = journal.select(selection()).unwrap();
    let name = filename(&selection().key).unwrap();
    let mut changed = attempt.record.clone();
    changed.selection.attempt_id = [9; 32];
    journal
        .directory
        .write_atomic(&name, &encode(&changed).unwrap(), PublishMode::Replace)
        .unwrap();
    assert!(matches!(
        journal.select_verification_originals(&mut attempt, vec![1], vec![2], 1_000),
        Err(Conflict)
    ));
    changed.scope = [9; 32];
    journal
        .directory
        .write_atomic(&name, &encode(&changed).unwrap(), PublishMode::Replace)
        .unwrap();
    assert!(matches!(journal.read(&selection().key), Err(Invalid)));
    journal
        .directory
        .write_atomic(&name, b"torn frame", PublishMode::Replace)
        .unwrap();
    assert!(matches!(journal.read(&selection().key), Err(Invalid)));
}

#[cfg(unix)]
#[test]
fn replaced_directory_lock_or_linked_record_refuses_all_operations() {
    use std::os::unix::fs::symlink;
    let (temp, _parent, journal) = initialized();
    let path = temp.path().join("issuer");
    symlink("absent", path.join(filename(&[9; 32]).unwrap())).unwrap();
    assert!(journal.read(&[9; 32]).is_err());
    std::fs::rename(&path, temp.path().join("old")).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(journal.read(&[9; 32]).is_err());
    drop(journal);
    let (_temp, _parent, journal) = initialized();
    let path = journal.directory.path().to_owned();
    std::fs::rename(path.join(LOCK), path.join("old.lock")).unwrap();
    let _replacement = journal.directory.create_lock(LOCK).unwrap();
    assert!(journal.read(&[9; 32]).is_err());
}

#[test]
fn failed_publication_poison_is_cleared_only_by_reopen_and_recovery() {
    for publish_first in [false, true] {
        let (temp, _parent, mut journal) = initialized();
        let attempt = journal.select(selection()).unwrap();
        let mut record = attempt.record.clone();
        record.phase = EnrollmentJournalPhaseV1::Verifying;
        record.request = vec![1];
        record.worker_request = vec![2];
        record.verification_time_ms = 1_000;
        record.worker_configuration = [9; 32];
        let outcome = journal.publish_with(
            &filename(&selection().key).unwrap(),
            &encode(&record).unwrap(),
            PublishMode::Replace,
            |directory, name, bytes, mode| {
                if publish_first {
                    directory.write_atomic(name, bytes, mode)?;
                }
                // Deterministic injection around the actual atomic publication boundary;
                // this is an ordering test, not a physical power-loss qualification.
                Err(io::Error::other("injected durability uncertainty"))
            },
        );
        assert_eq!(outcome, Err(Uncertain));
        assert!(matches!(journal.read(&selection().key), Err(Uncertain)));
        assert!(matches!(journal.select(selection()), Err(Uncertain)));
        drop(journal);
        let mut journal =
            EnrollmentJournalV1::open(&temp.path().join("issuer"), b"approved scope DATA").unwrap();
        let mut restored = journal.read(&selection().key).unwrap().unwrap();
        if publish_first {
            assert_eq!(restored.phase(), EnrollmentJournalPhaseV1::Verifying);
            assert!(matches!(
                journal.select_verification_originals(&mut restored, vec![1], vec![2], 1_000),
                Err(Conflict)
            ));
        } else {
            assert_eq!(restored.phase(), EnrollmentJournalPhaseV1::Selected);
            assert!(
                journal
                    .select_verification_originals(&mut restored, vec![1], vec![2], 1_000)
                    .is_ok()
            );
        }
    }
}
