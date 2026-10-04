//! Actual claim, phase publication refusal, unchanged private source and no-reroll controls.

use super::*;
use iroha_allocation::release::ReleaseRegistration;
use std::os::unix::fs::PermissionsExt as _;
use std::task::{Context, Waker};
const HANDLE: &str = "software://iroha/consensus-threshold/retained-attempt";

fn identities() -> (GlobalThresholdBeaconDkgSessionV1, Vec<KeyPair>, Vec<PeerId>) {
    let mut keys = (1..=4)
        .map(|index| KeyPair::try_from_seed(vec![index; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let roster = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"retained-attempt-network"),
    ));
    (
        GlobalThresholdBeaconDkgSessionV1 {
            version: 1,
            network_id,
            session_id: Hash::new(b"retained-attempt-session").into(),
            attempt_id: Hash::new(b"retained-attempt-once").into(),
            authority_generation: 0,
            roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
            committee_size: 4,
            threshold: 2,
            start_height: 1,
            commitments_end_height: 2,
            deliveries_end_height: 3,
            acceptances_end_height: 4,
        },
        keys,
        roster,
    )
}
fn root() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::Builder::new()
        .prefix(".retained-attempt-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = fs::canonicalize(temporary.path()).unwrap();
    (temporary, path)
}
fn prepare(
    root: &Path,
    budget: &AllocationBudget,
) -> std::result::Result<(SeatDkgAttempt, [File; 2]), AttemptError> {
    let (session, keys, roster) = identities();
    let clock = NativeJournalCursor::new(
        ChainId::from("retained-attempt"),
        session.network_id,
        iroha_data_model::block::consensus::SumeragiRootScope::Global,
        NativeFinalityLimits {
            block_bytes: NATIVE_FINALITY_MAX_BLOCK_BYTES,
            journal_bytes: NATIVE_FINALITY_MAX_JOURNAL_BYTES,
            block_count: NATIVE_FINALITY_MAX_BLOCK_COUNT,
            allocated_bytes: 128 * 1024 * 1024,
        },
        budget,
    )?;
    let (public_read, public_write) = rustix::pipe::pipe().unwrap();
    let (finality_read, finality_write) = rustix::pipe::pipe().unwrap();
    let attempt = SeatDkgAttempt::new(
        session,
        &roster,
        1,
        keys[0].clone(),
        File::from(public_read),
        File::from(finality_read),
        clock,
        10,
        HANDLE,
        7,
        root,
        Instant::now() + Duration::from_secs(60),
        budget,
    )?;
    Ok((
        attempt,
        [File::from(public_write), File::from(finality_write)],
    ))
}
fn through_publication_encoding(attempt: &mut SeatDkgAttempt) {
    for expected in [
        Phase::Claimed,
        Phase::JournalDurable,
        Phase::Generated,
        Phase::PublicationEncoded,
    ] {
        attempt.step().unwrap();
        assert_eq!(attempt.phase, expected);
    }
}

#[test]
fn local_attempt_claim_follows_complete_original_pool_preparation() {
    let (_temporary, root) = root();
    let floor = ReleaseRegistration::allocation_layout().size();
    let budget = AllocationBudget::new(8 * 1024 * 1024 + floor);
    let mut reservation = budget.try_reserve_bytes(floor).unwrap();
    let mut registration = ReleaseRegistration::from_reservation(&mut reservation).unwrap();
    drop(reservation);
    let blocker = budget
        .try_reserve_bytes(budget.limit_bytes() - floor)
        .unwrap();
    let error = match prepare(&root, &budget) {
        Ok(_) => panic!("occupied pool cannot prepare private work"),
        Err(error) => error,
    };
    let AttemptError::Local(LocalGlobalThresholdBeaconDkgErrorV1::Session(
        GlobalThresholdBeaconSessionError::Admission(AllocationRefusal::Capacity {
            release, ..
        }),
    )) = error
    else {
        panic!("original capacity source")
    };
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    let mut context = Context::from_waker(Waker::noop());
    assert!(registration.poll_wait(&release, &mut context).is_pending());
    let foreign = AllocationBudget::new(1);
    drop(foreign.try_reserve_bytes(1).unwrap());
    assert!(registration.poll_wait(&release, &mut context).is_pending());
    drop(blocker);
    assert!(registration.poll_wait(&release, &mut context).is_ready());
    registration.cancel();
    let (mut attempt, _writes) = prepare(&root, &budget).unwrap();
    assert_eq!(attempt.phase, Phase::Prepared);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert!(attempt.inputs.belongs_to(&budget));
    assert!(attempt.verifier.as_ref().unwrap().belongs_to(&budget));
    assert!(attempt.attempt_journal.belongs_to(&budget));
    assert!(attempt.provider_handle.belongs_to(&budget));
    attempt.step().unwrap();
    let directory = attempt.claim.directory().unwrap();
    assert_eq!(directory.file.metadata().unwrap().mode() & 0o7777, 0o700);
    let inode = directory.file.metadata().unwrap().ino();
    let child = directory.path.clone();
    drop(attempt);
    assert_eq!(budget.reserved_bytes(), floor);
    assert_eq!(fs::metadata(&child).unwrap().ino(), inode);
    let (mut repeated, _writes) = prepare(&root, &budget).unwrap();
    assert!(matches!(
        repeated.step(),
        Err(AttemptError::Claim(ClaimError::AlreadyClaimed(_)))
    ));
    assert!(repeated.prepared.is_some());
    assert!(repeated.local.is_none());
    drop(repeated);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn publication_refusal_resumes_same_generated_secret_frame_and_original_file_only() {
    let (_temporary, root) = root();
    let budget = AllocationBudget::new(8 * 1024 * 1024);
    let (mut attempt, _writes) = prepare(&root, &budget).unwrap();
    through_publication_encoding(&mut attempt);
    let local = attempt.local.as_ref().unwrap();
    let public_hash = local.publication_hash();
    let frame_pointer = local.encoded_public_frame().as_ptr();
    let frame = local.encoded_public_frame().to_vec();
    assert!(attempt.prepared.is_none());
    let directory = attempt.claim.directory().unwrap();
    let path = directory.path.join("publication.norito");
    fs::write(&path, b"existing unrelated file").unwrap();
    let held = budget.reserved_bytes();
    assert!(matches!(
        attempt.step(),
        Err(AttemptError::Export(seat_export::ExportError::Io(_)))
    ));
    assert_eq!(attempt.phase, Phase::PublicationEncoded);
    assert_eq!(
        attempt.local.as_ref().unwrap().publication_hash(),
        public_hash
    );
    assert_eq!(
        attempt
            .local
            .as_ref()
            .unwrap()
            .encoded_public_frame()
            .as_ptr(),
        frame_pointer
    );
    assert_eq!(
        attempt.local.as_ref().unwrap().encoded_public_frame(),
        frame
    );
    assert_eq!(budget.reserved_bytes(), held);
    fs::remove_file(&path).unwrap();
    attempt.step().unwrap();
    assert_eq!(attempt.phase, Phase::PublicationDurable);
    assert_eq!(fs::read(&path).unwrap(), frame);
    let inode = fs::metadata(&path).unwrap().ino();
    attempt.publish_phase(1).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
    assert_eq!(
        attempt.local.as_ref().unwrap().publication_hash(),
        public_hash
    );
    drop(attempt);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn pending_attempt_keeps_generated_private_owner_and_terminal_deadline_never_rerolls() {
    let (_temporary, root) = root();
    let budget = AllocationBudget::new(8 * 1024 * 1024);
    let (mut attempt, _writes) = prepare(&root, &budget).unwrap();
    through_publication_encoding(&mut attempt);
    let original = attempt.local.as_ref().unwrap().publication_hash();
    let retained = budget.reserved_bytes();
    // An already elapsed absolute deadline is a real pending-owner failure;
    // neither resumption nor the error carrier may reset it or generate again.
    attempt.deadline = Instant::now();
    let failure = attempt.resume().unwrap_err();
    assert_eq!(failure.owner.phase, Phase::Terminal);
    assert!(failure.owner.prepared.is_none());
    assert_eq!(
        failure.owner.local.as_ref().unwrap().publication_hash(),
        original
    );
    assert_eq!(budget.reserved_bytes(), retained);
    let second = failure.owner.resume().unwrap_err();
    assert_eq!(second.owner.phase, Phase::Terminal);
    assert_eq!(
        second.owner.local.as_ref().unwrap().publication_hash(),
        original
    );
    assert_eq!(budget.reserved_bytes(), retained);
    drop(second);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}
