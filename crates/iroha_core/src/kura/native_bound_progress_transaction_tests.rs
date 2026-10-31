//! Actual Windows strict-progress originals and canonical recovery boundaries.

use super::*;
use std::fs;

struct Fixture {
    directory: iroha_fs::OwnerDirectory,
    kura: Arc<Kura>,
}

impl Fixture {
    fn new() -> Self {
        let kura = Kura::blank_kura_for_testing();
        let private = iroha_fs::PrivateDirectory::open_or_create(
            kura.store_root().join("strict-progress-owner"),
        )
        .unwrap();
        let directory = iroha_fs::OwnerDirectory::open(private.path()).unwrap();
        Self { directory, kura }
    }

    fn path(&self, role: Role) -> PathBuf {
        let data = self.directory.path().join("progress.norito");
        let index = self.directory.path().join("progress.index");
        match role {
            Role::Data => data,
            Role::Index => index,
            Role::RewriteData => data.with_extension("norito.tmp"),
            Role::RewriteIndex => index.with_extension("index.tmp"),
            Role::Prepend => index.with_extension("index.prepend.tmp"),
            Role::Build => Kura::bound_progress_append_build_path(&index),
            Role::Intent => Kura::bound_progress_append_intent_path(&index),
        }
    }

    fn capture(&self) -> NativeBoundProgressTransaction<'_> {
        let mut slot = None;
        NativeBoundProgressTransaction::prepare_into(
            &mut slot,
            &self.kura,
            &self.path(Role::Data),
            &self.path(Role::Index),
        )
        .unwrap();
        slot.unwrap()
    }

    fn seed(&self, role: Role, bytes: &[u8]) {
        let path = self.path(role);
        let mut slot = None;
        self.directory
            .create_private_journal_into(path.file_name().unwrap(), &mut slot)
            .unwrap();
        let mut file = slot.as_ref().unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        drop(slot);
        self.directory.sync().unwrap();
    }

    fn seed_pair(&self, data: Role, index: Role, height: u64, bytes: &[u8]) {
        self.seed(data, bytes);
        let mut index_bytes = SidecarIndexLayout::base_header(height).to_vec();
        index_bytes.extend_from_slice(
            &SidecarIndexEntry {
                offset: 0,
                len: bytes.len() as u64,
            }
            .to_bytes(),
        );
        self.seed(index, &index_bytes);
    }

    fn payload(&self, height: u64) -> Vec<u8> {
        let mut index = File::open(self.path(Role::Index)).unwrap();
        let length = index.metadata().unwrap().len();
        let layout = SidecarIndexLayout::read_from(&mut index, length).unwrap();
        index
            .seek(SeekFrom::Start(layout.entry_position(height).unwrap()))
            .unwrap();
        let mut encoded = [0; PIPELINE_INDEX_ENTRY_SIZE];
        index.read_exact(&mut encoded).unwrap();
        let entry = SidecarIndexEntry::from_bytes(encoded);
        let mut data = File::open(self.path(Role::Data)).unwrap();
        data.seek(SeekFrom::Start(entry.offset)).unwrap();
        let mut bytes = vec![0; usize::try_from(entry.len).unwrap()];
        data.read_exact(&mut bytes).unwrap();
        bytes
    }
}

struct ResetFaults;

impl Drop for ResetFaults {
    fn drop(&mut self) {
        FAIL_NEXT_BOUND_PROGRESS_INTENT_FILE_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_BOUND_PROGRESS_APPEND_DATA_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_BOUND_PROGRESS_APPEND_INDEX_SYNC.with(|flag| flag.set(false));
        FAIL_BOUND_PROGRESS_INTENT_DIRECTORY_SYNC.with(|slot| slot.set(None));
        FAIL_AFTER_BOUND_PROGRESS_APPEND_BUILD_CALLS.with(|slot| slot.set(None));
        FAIL_NEXT_INDEXED_SIDECAR_DATA_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_INDEXED_SIDECAR_INDEX_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(false));
    }
}

fn original_id(
    transaction: &NativeBoundProgressTransaction<'_>,
    role: Role,
) -> iroha_fs::FileIdentity {
    transaction.storage.slots[role.index()].identity.unwrap()
}

fn ordinary_message(failure: BoundProgressIoFailure) -> String {
    match failure {
        BoundProgressIoFailure::Ordinary(Error::IO(error, _)) => error.to_string(),
        other => panic!("expected initiating ordinary storage error, got {other:?}"),
    }
}

#[test]
fn native_bound_append_prepend_replacement_and_exact_retry_share_canonical_protocol() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    transaction
        .append(3, 3, b"three", "strict fixture")
        .unwrap();
    transaction.append(1, 1, b"one", "strict fixture").unwrap();
    transaction.append(2, 2, b"two", "strict fixture").unwrap();
    assert_eq!(fixture.payload(1), b"one");
    assert_eq!(fixture.payload(2), b"two");
    assert_eq!(fixture.payload(3), b"three");
    let before = fs::read(fixture.path(Role::Data)).unwrap();
    transaction.append(2, 2, b"two", "strict fixture").unwrap();
    assert_eq!(fs::read(fixture.path(Role::Data)).unwrap(), before);
    transaction
        .append(2, 2, b"replacement", "strict fixture")
        .unwrap();
    assert_eq!(fixture.payload(2), b"replacement");
    assert_eq!(fixture.payload(1), b"one");
    assert_eq!(fixture.payload(3), b"three");
    assert!(transaction.pending.is_none());
    assert!(!fixture.path(Role::Build).exists());
    assert!(!fixture.path(Role::Intent).exists());
    transaction.storage.verify(&transaction.namespace).unwrap();
}

#[test]
fn native_bound_creator_failure_retains_slot_before_main_mutation() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    transaction.storage.fail_at = Some(Step::Created(Role::Build));
    let failure = transaction
        .append(1, 1, b"one", "creator failure")
        .unwrap_err();
    assert_eq!(
        ordinary_message(failure),
        "injected retained strict progress boundary"
    );
    let identity = original_id(&transaction, Role::Build);
    assert!(
        transaction.storage.slots[Role::Build.index()]
            .original
            .is_some()
    );
    assert_eq!(
        iroha_fs::FileIdentity::of(&File::open(fixture.path(Role::Build)).unwrap()).unwrap(),
        identity
    );
    assert!(!fixture.path(Role::Data).exists());
    assert!(!fixture.path(Role::Index).exists());
    assert!(
        transaction
            .append(2, 2, b"different", "foreign retry")
            .is_err()
    );
    transaction.append(1, 1, b"one", "creator retry").unwrap();
    assert_eq!(fixture.payload(1), b"one");
}

#[test]
fn native_bound_intent_publication_failure_latches_original_before_main_writes() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    transaction.storage.fail_at = Some(Step::Published(Role::Build));
    assert_eq!(
        ordinary_message(
            transaction
                .append(1, 1, b"one", "intent failure")
                .unwrap_err()
        ),
        "injected retained strict progress boundary"
    );
    let build = &transaction.storage.slots[Role::Build.index()];
    assert!(build.published);
    assert_eq!(build.destination, Some(Role::Intent));
    let identity = build.identity.unwrap();
    assert_eq!(
        iroha_fs::FileIdentity::of(&File::open(fixture.path(Role::Intent)).unwrap()).unwrap(),
        identity
    );
    assert!(!fixture.path(Role::Build).exists());
    assert!(!fixture.path(Role::Data).exists());
    assert!(!fixture.path(Role::Index).exists());
    transaction.append(1, 1, b"one", "intent retry").unwrap();
    assert_eq!(fixture.payload(1), b"one");
}

#[test]
fn native_bound_intent_sync_error_survives_original_cleanup() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    fail_next_bound_progress_intent_file_sync_for_tests();
    let message = ordinary_message(transaction.append(1, 1, b"one", "intent sync").unwrap_err());
    assert!(message.contains("injected"));
    assert!(message.contains("intent"));
    assert!(!fixture.path(Role::Data).exists());
    assert!(!fixture.path(Role::Index).exists());
    assert!(transaction.storage.slots[Role::Build.index()].closed);
    transaction
        .append(1, 1, b"one", "intent sync retry")
        .unwrap();
    assert_eq!(fixture.payload(1), b"one");
}

#[test]
fn native_bound_payload_and_index_failures_roll_forward_same_originals() {
    let _reset = ResetFaults;
    let injections: [fn(); 2] = [
        fail_next_bound_progress_append_data_sync_for_tests,
        fail_next_bound_progress_append_index_sync_for_tests,
    ];
    for inject in injections {
        let fixture = Fixture::new();
        let mut transaction = fixture.capture();
        inject();
        assert!(matches!(
            transaction.append(7, 7, b"seven", "append barrier"),
            Err(BoundProgressIoFailure::Ordinary(_))
        ));
        let identities = [
            original_id(&transaction, Role::Data),
            original_id(&transaction, Role::Index),
        ];
        assert!(fixture.path(Role::Intent).exists());
        transaction
            .append(7, 7, b"seven", "append barrier retry")
            .unwrap();
        assert_eq!(fixture.payload(7), b"seven");
        assert_eq!(fs::read(fixture.path(Role::Data)).unwrap(), b"seven");
        assert_eq!(
            [
                original_id(&transaction, Role::Data),
                original_id(&transaction, Role::Index)
            ],
            identities
        );
    }
}

#[test]
fn native_bound_corrupt_complete_suffix_rolls_back_using_authenticated_preimage() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    transaction.append(1, 1, b"old", "seed").unwrap();
    fail_next_bound_progress_append_data_sync_for_tests();
    assert!(transaction.append(2, 2, b"new", "partial append").is_err());
    let mut original = transaction
        .storage
        .cloned(&transaction.namespace, Role::Data)
        .unwrap()
        .unwrap();
    original.seek(SeekFrom::Start(3)).unwrap();
    original.write_all(b"bad").unwrap();
    original.sync_all().unwrap();
    drop(original);
    transaction.recover("corrupt suffix recovery").unwrap();
    assert_eq!(fs::read(fixture.path(Role::Data)).unwrap(), b"old");
    assert_eq!(fixture.payload(1), b"old");
    transaction
        .append(2, 2, b"new", "same request retry")
        .unwrap();
    assert_eq!(fixture.payload(2), b"new");
}

#[test]
fn native_bound_absent_rollback_keeps_close_tombstone_and_refuses_foreign_name() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    fail_next_bound_progress_append_data_sync_for_tests();
    assert!(transaction.append(1, 1, b"one", "initial partial").is_err());
    transaction
        .storage
        .cloned(&transaction.namespace, Role::Data)
        .unwrap()
        .unwrap()
        .set_len(1)
        .unwrap();
    transaction.storage.fail_at = Some(Step::Closed(Role::Index));
    assert_eq!(
        ordinary_message(transaction.recover("rollback close failure").unwrap_err()),
        "injected retained strict progress boundary"
    );
    let index = &transaction.storage.slots[Role::Index.index()];
    assert!(index.disposed && index.closed && index.original.is_none());
    assert!(!fixture.path(Role::Index).exists());
    fixture.seed(Role::Index, b"foreign");
    assert!(transaction.recover("foreign name refusal").is_err());
    assert_eq!(fs::read(fixture.path(Role::Index)).unwrap(), b"foreign");
    assert!(
        transaction.storage.slots[Role::Data.index()]
            .original
            .is_some()
    );
    assert!(fixture.path(Role::Intent).exists());
}

#[test]
fn native_bound_disposition_error_retains_original_until_retry_closes_once() {
    let fixture = Fixture::new();
    fixture.seed(Role::Build, b"unpublished");
    let mut transaction = fixture.capture();
    transaction.storage.fail_at = Some(Step::Disposed(Role::Build));
    assert!(transaction.recover("dispose failure").is_err());
    let build = &transaction.storage.slots[Role::Build.index()];
    assert!(build.disposed && !build.closed && build.original.is_some());
    fixture
        .directory
        .require_retired_private_journal(build.original.as_ref().unwrap())
        .unwrap();
    assert_eq!(
        iroha_fs::FileIdentity::of(build.original.as_ref().unwrap()).unwrap(),
        build.identity.unwrap()
    );
    transaction.recover("dispose retry").unwrap();
    assert!(
        transaction.storage.slots[Role::Build.index()]
            .original
            .is_none()
    );
    assert!(!transaction.storage.slots[Role::Build.index()].disposed);
    assert!(!fixture.path(Role::Build).exists());
}

#[test]
fn native_bound_rewrite_publication_retries_keep_displaced_predecessors() {
    for phase in [Role::RewriteData, Role::RewriteIndex] {
        let fixture = Fixture::new();
        fixture.seed_pair(Role::Data, Role::Index, 1, b"old");
        fixture.seed_pair(Role::RewriteData, Role::RewriteIndex, 1, b"replacement");
        let mut transaction = fixture.capture();
        let original_data = original_id(&transaction, Role::Data);
        transaction.storage.fail_at = Some(Step::Published(phase));
        assert!(transaction.recover("rewrite publication boundary").is_err());
        assert!(transaction.storage.slots[phase.index()].published);
        let old = &transaction.storage.slots[Role::Data.index()];
        assert_eq!(
            iroha_fs::FileIdentity::of(old.original.as_ref().unwrap()).unwrap(),
            original_data
        );
        fixture
            .directory
            .require_retired_private_journal(old.original.as_ref().unwrap())
            .unwrap();
        transaction.recover("rewrite publication retry").unwrap();
        assert_eq!(fixture.payload(1), b"replacement");
        assert!(!fixture.path(Role::RewriteData).exists());
        assert!(!fixture.path(Role::RewriteIndex).exists());
    }
}

#[test]
fn native_bound_malformed_intent_and_index_only_rewrite_preserve_evidence() {
    for role in [Role::Intent, Role::RewriteIndex] {
        let fixture = Fixture::new();
        fixture.seed_pair(Role::Data, Role::Index, 1, b"old");
        fixture.seed(role, b"malformed");
        let before = [
            fs::read(fixture.path(Role::Data)).unwrap(),
            fs::read(fixture.path(Role::Index)).unwrap(),
            fs::read(fixture.path(role)).unwrap(),
        ];
        let mut transaction = fixture.capture();
        assert_eq!(
            transaction
                .recover("malformed state")
                .unwrap_err()
                .classification(),
            BoundProgressRecoveryFailure::InvalidData
        );
        assert_eq!(
            [
                fs::read(fixture.path(Role::Data)).unwrap(),
                fs::read(fixture.path(Role::Index)).unwrap(),
                fs::read(fixture.path(role)).unwrap()
            ],
            before
        );
    }
}

#[test]
fn native_bound_incomplete_both_file_rewrite_discards_only_original_temps() {
    let fixture = Fixture::new();
    fixture.seed_pair(Role::Data, Role::Index, 1, b"old");
    fixture.seed(Role::RewriteData, b"uncommitted");
    fixture.seed(Role::RewriteIndex, b"incomplete");
    let mut transaction = fixture.capture();
    let identities = [
        original_id(&transaction, Role::Data),
        original_id(&transaction, Role::Index),
    ];
    transaction.recover("incomplete rewrite").unwrap();
    assert_eq!(fixture.payload(1), b"old");
    assert_eq!(
        [
            original_id(&transaction, Role::Data),
            original_id(&transaction, Role::Index)
        ],
        identities
    );
    assert!(!fixture.path(Role::RewriteData).exists());
    assert!(!fixture.path(Role::RewriteIndex).exists());
}

#[test]
fn native_bound_capture_refuses_hardlink_and_existing_writer_without_replacement() {
    for hardlink in [false, true] {
        let fixture = Fixture::new();
        fixture.seed_pair(Role::Data, Role::Index, 1, b"old");
        let writer = if hardlink {
            fs::hard_link(
                fixture.path(Role::Data),
                fixture.directory.path().join("alias"),
            )
            .unwrap();
            None
        } else {
            Some(
                fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(fixture.path(Role::Data))
                    .unwrap(),
            )
        };
        let mut slot = None;
        assert!(
            NativeBoundProgressTransaction::prepare_into(
                &mut slot,
                &fixture.kura,
                &fixture.path(Role::Data),
                &fixture.path(Role::Index)
            )
            .is_err()
        );
        assert!(slot.is_some());
        assert_eq!(fs::read(fixture.path(Role::Data)).unwrap(), b"old");
        drop(writer);
    }
}

#[test]
fn native_bound_retry_refuses_another_kura_and_namespace() {
    let fixture = Fixture::new();
    let other = Fixture::new();
    let mut slot = Some(fixture.capture());
    assert!(
        NativeBoundProgressTransaction::prepare_into(
            &mut slot,
            &other.kura,
            &fixture.path(Role::Data),
            &fixture.path(Role::Index)
        )
        .is_err()
    );
    assert!(
        NativeBoundProgressTransaction::prepare_into(
            &mut slot,
            &fixture.kura,
            &fixture.directory.path().join("other.data"),
            &fixture.path(Role::Index)
        )
        .is_err()
    );
    assert!(slot.as_ref().unwrap().pending.is_none());
}

#[test]
fn native_bound_first_ordinary_error_survives_later_classification() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    transaction.storage.retain_failure(Error::IO(
        io::Error::other("first original storage cause"),
        fixture.path(Role::Data),
    ));
    transaction
        .storage
        .retain_classification(BoundProgressRecoveryFailure::InvalidData);
    transaction.storage.retain_failure(Error::IO(
        io::Error::other("later cleanup"),
        fixture.path(Role::Index),
    ));
    assert_eq!(
        ordinary_message(transaction.failed(BoundProgressRecoveryFailure::InvalidData)),
        "first original storage cause"
    );
}

#[test]
fn native_bound_unstarted_forward_gap_does_not_poison_the_owner() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    transaction.append(1, 1, b"one", "valid seed").unwrap();
    let bytes = fs::read(fixture.path(Role::Data)).unwrap();
    let identities = [
        original_id(&transaction, Role::Data),
        original_id(&transaction, Role::Index),
    ];
    assert_eq!(
        transaction
            .append(
                MAX_INDEXED_SIDECAR_GAP_ENTRIES + 3,
                1,
                b"invalid",
                "forward gap"
            )
            .unwrap_err()
            .classification(),
        BoundProgressRecoveryFailure::InvalidData
    );
    assert!(transaction.pending.is_none());
    assert!(!transaction.storage.mutation_started);
    assert_eq!(fs::read(fixture.path(Role::Data)).unwrap(), bytes);
    assert!(!fixture.path(Role::Build).exists());
    assert!(!fixture.path(Role::Intent).exists());
    transaction
        .append(2, 2, b"two", "valid after refusal")
        .unwrap();
    assert_eq!(fixture.payload(2), b"two");
    assert_eq!(
        [
            original_id(&transaction, Role::Data),
            original_id(&transaction, Role::Index)
        ],
        identities
    );
}

#[test]
fn native_bound_unstarted_backward_gap_does_not_poison_the_owner() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    let height = MAX_INDEXED_SIDECAR_GAP_ENTRIES + 2;
    transaction
        .append(height, height, b"last", "valid high seed")
        .unwrap();
    let bytes = fs::read(fixture.path(Role::Data)).unwrap();
    assert_eq!(
        transaction
            .append(1, 1, b"invalid", "backward gap")
            .unwrap_err()
            .classification(),
        BoundProgressRecoveryFailure::InvalidData
    );
    assert!(transaction.pending.is_none());
    assert!(!transaction.storage.mutation_started);
    assert_eq!(fs::read(fixture.path(Role::Data)).unwrap(), bytes);
    transaction
        .append(height - 1, height - 1, b"previous", "valid prepend")
        .unwrap();
    assert_eq!(fixture.payload(height - 1), b"previous");
    assert_eq!(fixture.payload(height), b"last");
}

#[test]
fn native_bound_unstarted_absent_gap_leaves_no_artifact_or_pending_request() {
    let fixture = Fixture::new();
    let mut transaction = fixture.capture();
    assert_eq!(
        transaction
            .append(
                MAX_INDEXED_SIDECAR_GAP_ENTRIES + 2,
                1,
                b"invalid",
                "absent gap"
            )
            .unwrap_err()
            .classification(),
        BoundProgressRecoveryFailure::InvalidData
    );
    assert!(transaction.pending.is_none());
    assert!(!transaction.storage.mutation_started);
    assert!(ROLES.into_iter().all(|role| !fixture.path(role).exists()));
    transaction
        .append(1, 1, b"one", "valid absent retry")
        .unwrap();
    assert_eq!(fixture.payload(1), b"one");
}

#[test]
fn native_bound_failed_recovery_keeps_the_original_pending_request() {
    let fixture = Fixture::new();
    fixture.seed(Role::Intent, b"malformed");
    let mut transaction = fixture.capture();
    assert_eq!(
        transaction
            .append(1, 1, b"one", "failed recovery")
            .unwrap_err()
            .classification(),
        BoundProgressRecoveryFailure::InvalidData
    );
    let pending = transaction.pending.as_ref().unwrap();
    assert_eq!(
        (pending.height, pending.payload.as_slice()),
        (1, b"one".as_slice())
    );
    assert!(
        transaction
            .append(2, 2, b"two", "different request")
            .is_err()
    );
    assert_eq!(fs::read(fixture.path(Role::Intent)).unwrap(), b"malformed");
    assert!(!fixture.path(Role::Data).exists());
}

#[test]
fn native_bound_normalized_rewrite_reissues_failed_final_barrier() {
    let _reset = ResetFaults;
    let fixture = Fixture::new();
    fixture.seed_pair(Role::Data, Role::Index, 1, b"old");
    fixture.seed_pair(Role::RewriteData, Role::RewriteIndex, 1, b"replacement");
    let mut transaction = fixture.capture();
    // Rewrite publication uses the ordinary sidecar barriers. The second
    // intent-directory barrier in finish is after both original-slot moves.
    fail_bound_progress_intent_directory_sync_for_tests(1, 0);
    assert_eq!(
        ordinary_message(transaction.recover("late final barrier").unwrap_err()),
        "injected bound progress append-intent directory sync failure"
    );
    assert!(transaction.storage.final_barrier_pending);
    assert!(
        transaction.storage.slots.iter().all(|slot| !slot.published
            && !slot.disposed
            && !slot.closed
            && !slot.displaced_closed)
    );
    let identities = [
        original_id(&transaction, Role::Data),
        original_id(&transaction, Role::Index),
    ];
    assert_eq!(fixture.payload(1), b"replacement");
    fail_bound_progress_intent_directory_sync_for_tests(1, 0);
    assert_eq!(
        ordinary_message(transaction.recover("late final barrier retry").unwrap_err()),
        "injected bound progress append-intent directory sync failure"
    );
    assert!(transaction.storage.final_barrier_pending);
    assert_eq!(
        [
            original_id(&transaction, Role::Data),
            original_id(&transaction, Role::Index)
        ],
        identities
    );
    transaction.recover("final barrier accepted").unwrap();
    assert!(!transaction.storage.final_barrier_pending);
    assert_eq!(
        [
            original_id(&transaction, Role::Data),
            original_id(&transaction, Role::Index)
        ],
        identities
    );
    assert_eq!(fixture.payload(1), b"replacement");
}
