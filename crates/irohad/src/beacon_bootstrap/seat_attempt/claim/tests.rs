//! Real exclusive creation, named-inode replacement and exact prepared path custody.

use super::*;
use std::{fs, os::unix::fs::PermissionsExt as _, time::Duration};

fn root() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::Builder::new()
        .prefix(".beacon-attempt-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = fs::canonicalize(temporary.path()).unwrap();
    (temporary, path)
}
fn prepare(root: &Path, budget: &AllocationBudget) -> PreparedAttemptClaim {
    PreparedAttemptClaim::new(
        root,
        &[0x38; 32],
        31,
        Instant::now() + Duration::from_secs(30),
        budget,
    )
    .unwrap_or_else(|error| panic!("prepared exact claim: {error}"))
}

#[test]
fn exact_paths_are_prepared_before_claim_and_stay_with_the_original_attempt() {
    let (_temporary, root) = root();
    let name = Name::new(&[0x38; 32], 31).unwrap();
    let root_len = root.as_os_str().as_bytes().len();
    let child_len = root_len + 1 + name.as_str().len();
    let ledger = Layout::array::<AllocationCharge>(1).unwrap().size();
    let budget = AllocationBudget::new(root_len + child_len + 2 * ledger);
    let mut claim = prepare(&root, &budget);
    assert!(!root.join(name.as_str()).exists());
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    let original_path = claim.child_path.as_ref().unwrap().bytes.as_slice().as_ptr();
    claim.make_durable().unwrap();
    let held = claim.directory().unwrap();
    assert_eq!(held.path.as_os_str().as_bytes().as_ptr(), original_path);
    let inode = held.file.metadata().unwrap().ino();
    assert_eq!(held.file.metadata().unwrap().mode() & 0o7777, 0o700);
    claim.make_durable().unwrap();
    assert_eq!(
        claim.directory().unwrap().file.metadata().unwrap().ino(),
        inode
    );
    let directory = claim.child.as_ref().unwrap();
    assert_eq!(directory.allocation_bytes(), Some(child_len + ledger));
    assert!(directory.belongs_to(&budget));
    assert_eq!(
        directory.get().path.as_os_str().as_bytes().as_ptr(),
        original_path
    );
    assert_eq!(budget.reserved_bytes(), root_len + child_len + 2 * ledger);
    drop(claim);
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(
        root.join(name.as_str()).is_dir(),
        "completion/drop never removes the no-reroll claim"
    );
}

#[test]
fn original_path_refusal_precedes_mkdir_and_existing_claim_is_never_adopted() {
    let (_temporary, root) = root();
    let name = Name::new(&[0x38; 32], 31).unwrap();
    let closed = AllocationBudget::new(0);
    assert!(matches!(
        PreparedAttemptClaim::new(
            &root,
            &[0x38; 32],
            31,
            Instant::now() + Duration::from_secs(30),
            &closed
        ),
        Err(ClaimError::Admission(_))
    ));
    assert!(!root.join(name.as_str()).exists());
    assert_eq!(closed.reserved_bytes(), 0);
    let budget = AllocationBudget::new(8192);
    let mut original = prepare(&root, &budget);
    original.make_durable().unwrap();
    let inode = original.directory().unwrap().file.metadata().unwrap().ino();
    let mut second = prepare(&root, &budget);
    let ClaimError::AlreadyClaimed(error) = second.make_durable().unwrap_err() else {
        panic!("existing exact attempt cannot be resumed by a new owner");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(second.terminal);
    assert!(second.child.is_none());
    assert_eq!(
        original.directory().unwrap().file.metadata().unwrap().ino(),
        inode
    );
    drop(second);
    drop(original);
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(root.join(name.as_str()).is_dir());
}

#[test]
fn replaced_newly_created_child_cannot_be_pinned_as_the_original_attempt() {
    let (_temporary, root) = root();
    let budget = AllocationBudget::new(8192);
    let mut claim = prepare(&root, &budget);
    claim.create().unwrap();
    assert!(claim.mkdir_completed);
    let original = claim.created.unwrap().inode;
    let path = root.join(claim.name.as_str());
    let retained = root.join("original-created-child");
    fs::rename(&path, &retained).unwrap();
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    assert_ne!(fs::metadata(&path).unwrap().ino(), original);
    assert!(matches!(claim.make_durable(), Err(ClaimError::Custody)));
    assert!(claim.terminal);
    assert!(claim.child.is_none());
    assert_eq!(fs::metadata(&retained).unwrap().ino(), original);
    assert!(matches!(claim.make_durable(), Err(ClaimError::Custody)));
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    drop(claim);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn expired_unsafe_symlink_and_noncanonical_roots_never_create_attempts() {
    let (_temporary, root) = root();
    let budget = AllocationBudget::new(8192);
    let mut expired = PreparedAttemptClaim::new(&root, &[0x38; 32], 31, Instant::now(), &budget)
        .unwrap_or_else(|error| panic!("expired claim owner: {error}"));
    assert!(matches!(expired.make_durable(), Err(ClaimError::Deadline)));
    assert!(!expired.mkdir_completed);
    assert!(!root.join(expired.name.as_str()).exists());
    drop(expired);
    let alias = root.join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    assert!(
        PreparedAttemptClaim::new(
            &alias,
            &[0x38; 32],
            31,
            Instant::now() + Duration::from_secs(30),
            &budget
        )
        .is_err()
    );
    let noncanonical = PathBuf::from(format!("{}//", root.display()));
    assert!(matches!(
        PreparedAttemptClaim::new(
            &noncanonical,
            &[0x38; 32],
            31,
            Instant::now() + Duration::from_secs(30),
            &budget
        ),
        Err(ClaimError::Custody)
    ));
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        PreparedAttemptClaim::new(
            &root,
            &[0x38; 32],
            31,
            Instant::now() + Duration::from_secs(30),
            &budget
        ),
        Err(ClaimError::Custody)
    ));
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
}

#[cfg(target_os = "macos")]
#[test]
fn claim_device_identity_preserves_the_platform_signed_device_bits() {
    assert_eq!(device_identity_from_raw(-1), u64::MAX);
    assert_eq!(
        device_identity_from_raw(i32::MIN),
        u64::MAX - u64::try_from(i32::MAX).unwrap(),
    );
}

#[test]
fn claim_stat_and_owned_descriptor_have_the_same_exact_filesystem_identity() {
    let (_temporary, root) = root();
    let budget = AllocationBudget::new(8192);
    let mut claim = prepare(&root, &budget);
    claim.create().unwrap();
    let created = claim.created.unwrap();
    claim.pin().unwrap();
    let metadata = claim.child.as_ref().unwrap().get().file.metadata().unwrap();
    assert_eq!(created.device, metadata.dev());
    assert_eq!(created.inode, metadata.ino());
    drop(claim);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn one_shot_attempt_directory_cannot_reroll_after_restart() {
    let (_temporary, root) = root();
    let budget = AllocationBudget::new(8192);
    let attempt = iroha_crypto::Hash::new(b"one-shot-attempt");
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut first = PreparedAttemptClaim::new(&root, attempt.as_ref(), 1, deadline, &budget)
        .unwrap_or_else(|error| panic!("exact seat: {error}"));
    first.make_durable().unwrap();
    let directory = first.directory().unwrap();
    assert_eq!(
        directory.path.file_name().unwrap(),
        std::ffi::OsStr::new(Name::new(attempt.as_ref(), 1).unwrap().as_str())
    );
    let mut publication = super::super::publication::PhasePublication::new(
        super::super::publication::PhaseFile::Journal,
    );
    publication.publish(directory, b"{}").unwrap();
    assert!(publication.complete());
    drop(publication);
    drop(first);
    let mut repeated = PreparedAttemptClaim::new(&root, attempt.as_ref(), 1, deadline, &budget)
        .unwrap_or_else(|error| panic!("prepare same consumed seat: {error}"));
    assert!(
        matches!(repeated.make_durable(), Err(ClaimError::AlreadyClaimed(_))),
        "the same attempt must not generate fresh randomness after restart"
    );
    let mut second_seat = PreparedAttemptClaim::new(&root, attempt.as_ref(), 2, deadline, &budget)
        .unwrap_or_else(|error| panic!("distinct original seat: {error}"));
    second_seat.make_durable().unwrap();
    assert!(second_seat.directory().is_some());
    drop(repeated);
    drop(second_seat);
    assert_eq!(budget.reserved_bytes(), 0);
}
