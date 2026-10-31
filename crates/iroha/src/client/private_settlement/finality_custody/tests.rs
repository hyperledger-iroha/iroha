//! Actual private-file custody over native-certified synthetic execution fixtures.
//! These tests grant no monetary outcome; the observer must verify exact effects before pinning.
use super::*;
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;

fn binding(
    owner: &Arc<FilePrivateSettlementFinality>,
    native: &NativeFinalityFixture,
) -> BoundPrivateSettlementFinality {
    owner
        .bind(
            WorkflowId::from_bytes([11; 32]),
            [12; 32],
            &native.verifier(),
        )
        .unwrap()
}
fn identity(seed: u8) -> TransactionIdentityV1 {
    TransactionIdentityV1 {
        signed_transaction_hash: [seed; 32],
        entrypoint_hash: [seed.wrapping_add(1); 32],
    }
}
fn prefix(native: &NativeFinalityFixture, target: u64) -> PendingFinalityRecord {
    PendingFinalityRecord {
        prefix: Some(PendingFinalityPrefix {
            lookup: FinalityLookup {
                phase: FinalityPhase::Register,
                identity: Some(identity(13)),
                height: target,
            },
            checkpoint: native.checkpoint(),
        }),
        ..Default::default()
    }
}

#[test]
fn pending_finality_fixed_slots_reopen_atomically_and_refuse_stale_cas() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("finality");
    let mut native = NativeFinalityFixture::new();
    let owner =
        Arc::new(FilePrivateSettlementFinality::open(&path, &[15; 32], 2 * 1024 * 1024).unwrap());
    let selected = binding(&owner, &native);
    assert!(selected.load().unwrap().digest.is_none());
    assert_eq!(
        std::fs::read_dir(&path).unwrap().count(),
        0,
        "fresh readonly custody creates no checkpoint or lock file"
    );
    let first = prefix(&native, 4);
    let digest = selected.compare_and_publish(None, &first).unwrap();
    let original_checkpoint = native.checkpoint();
    let block = native.block_with_submitted_work(native.next_header());
    native.certify(block);
    let mut second = prefix(&native, 4);
    second.register = Some(PendingFinalityPin {
        identity: identity(13),
        checkpoint: original_checkpoint,
    });
    second.terminal = Some(PendingFinalityTerminal {
        kind: FinalityTerminalKind::Settled,
        pin: PendingFinalityPin {
            identity: identity(17),
            checkpoint: native.checkpoint(),
        },
    });
    let newest = selected
        .compare_and_publish(Some(&digest), &second)
        .unwrap();
    assert_ne!(newest, digest);
    assert!(matches!(
        selected.compare_and_publish(Some(&digest), &first),
        Err(Error::Conflict)
    ));
    let encoded = std::fs::read(path.join(selected.name())).unwrap();
    assert_ne!(
        encoded,
        *encode_record(&second, MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES).unwrap()
    );
    drop(selected);
    drop(owner);
    let reopened =
        Arc::new(FilePrivateSettlementFinality::open(&path, &[15; 32], 2 * 1024 * 1024).unwrap());
    let selected = binding(&reopened, &native);
    let restored = selected.load().unwrap();
    assert_eq!(restored.digest, Some(newest));
    assert_eq!(
        *encode_record(&restored.record, MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES).unwrap(),
        *encode_record(&second, MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES).unwrap()
    );
    assert_eq!(restored.record.register.unwrap().identity, identity(13));
    assert_eq!(restored.record.terminal.unwrap().pin.identity, identity(17));
    assert_eq!(std::fs::read(path.join(selected.name())).unwrap(), encoded);
    assert_eq!(
        std::fs::read_dir(&path).unwrap().count(),
        2,
        "one fixed record and its original CAS lock"
    );
}

#[test]
fn pending_finality_custody_binds_key_workflow_request_and_independent_root() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("finality");
    let native = NativeFinalityFixture::new();
    let owner =
        Arc::new(FilePrivateSettlementFinality::open(&path, &[19; 32], 2 * 1024 * 1024).unwrap());
    let selected = binding(&owner, &native);
    let record = prefix(&native, 2);
    selected.compare_and_publish(None, &record).unwrap();
    let original = std::fs::read(path.join(selected.name())).unwrap();
    let changed_request = owner
        .bind(
            WorkflowId::from_bytes([11; 32]),
            [20; 32],
            &native.verifier(),
        )
        .unwrap();
    assert!(matches!(changed_request.load(), Err(Error::Binding)));
    let changed_workflow = owner
        .bind(
            WorkflowId::from_bytes([21; 32]),
            [12; 32],
            &native.verifier(),
        )
        .unwrap();
    owner
        .directory
        .write_atomic(changed_workflow.name(), &original, PublishMode::CreateNew)
        .unwrap();
    let refusal = changed_workflow
        .load()
        .err()
        .expect("foreign workflow refuses");
    assert!(matches!(&refusal, Error::Binding), "{refusal:?}");
    let wrong_key =
        Arc::new(FilePrivateSettlementFinality::open(&path, &[22; 32], 2 * 1024 * 1024).unwrap());
    assert!(matches!(
        binding(&wrong_key, &native).load(),
        Err(Error::Binding)
    ));
    let foreign = NativeFinalityFixture::start("foreign pending root");
    assert!(matches!(
        binding(&owner, &foreign).load(),
        Err(Error::Binding)
    ));
    let mut corrupt = original.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    owner
        .directory
        .write_atomic(selected.name(), &corrupt, PublishMode::Replace)
        .unwrap();
    assert!(matches!(selected.load(), Err(Error::Binding)));
    assert!(
        matches!(
            selected.compare_and_publish(None, &record),
            Err(Error::Binding)
        ),
        "corrupt custody cannot be treated as a missing prefix"
    );
    assert_eq!(std::fs::read(path.join(selected.name())).unwrap(), corrupt);
    let foreign_purpose = owner
        .cipher
        .encrypt_easy(
            b"contexts".as_slice(),
            encode_record(&record, MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES)
                .unwrap()
                .as_slice(),
        )
        .unwrap();
    owner
        .directory
        .write_atomic(selected.name(), &foreign_purpose, PublishMode::Replace)
        .unwrap();
    assert!(matches!(selected.load(), Err(Error::Binding)));
}

#[test]
fn pending_finality_aggregate_cap_refuses_before_publication_without_deleting_originals() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("finality");
    let native = NativeFinalityFixture::new();
    let record = prefix(&native, 2);
    let length = encode_record(&record, MAX_PRIVATE_SETTLEMENT_FINALITY_BYTES)
        .unwrap()
        .len();
    assert!(matches!(
        FilePrivateSettlementFinality::open(&path, &[23; 32], 0),
        Err(Error::Capacity)
    ));
    assert!(matches!(
        FilePrivateSettlementFinality::open(&path, &[0; 32], length),
        Err(Error::Binding)
    ));
    let owner = Arc::new(FilePrivateSettlementFinality::open(&path, &[23; 32], length).unwrap());
    assert!(matches!(
        owner.with_maximum_bytes(length + 1),
        Err(Error::Capacity)
    ));
    let constrained = Arc::new(owner.with_maximum_bytes(length - 1).unwrap());
    assert!(matches!(
        binding(&constrained, &native).compare_and_publish(None, &record),
        Err(Error::Capacity)
    ));
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    let selected = binding(&owner, &native);
    let digest = selected.compare_and_publish(None, &record).unwrap();
    let original = std::fs::read(path.join(selected.name())).unwrap();
    assert!(binding(&constrained, &native).load().is_err());
    let mut expanded = record.clone();
    expanded.register = Some(PendingFinalityPin {
        identity: identity(25),
        checkpoint: native.checkpoint(),
    });
    assert!(matches!(
        selected.compare_and_publish(Some(&digest), &expanded),
        Err(Error::Capacity)
    ));
    assert_eq!(std::fs::read(path.join(selected.name())).unwrap(), original);
    assert_eq!(selected.load().unwrap().digest, Some(digest));
}

#[test]
fn pending_finality_lookup_shape_and_original_interprocess_lock_are_enforced() {
    let root = tempfile::tempdir().unwrap();
    let native = NativeFinalityFixture::new();
    let owner = Arc::new(
        FilePrivateSettlementFinality::open(
            root.path().join("finality"),
            &[27; 32],
            2 * 1024 * 1024,
        )
        .unwrap(),
    );
    let selected = binding(&owner, &native);
    let mut invalid = prefix(&native, 2);
    invalid.prefix.as_mut().unwrap().lookup.height = 1;
    assert!(matches!(
        selected.compare_and_publish(None, &invalid),
        Err(Error::Binding)
    ));
    invalid = prefix(&native, 2);
    invalid.prefix.as_mut().unwrap().lookup.identity = None;
    assert!(matches!(
        selected.compare_and_publish(None, &invalid),
        Err(Error::Binding)
    ));
    let lock = owner.directory.open_lock(selected.lock_name()).unwrap();
    lock.try_lock().unwrap();
    assert!(matches!(
        selected.compare_and_publish(None, &prefix(&native, 2)),
        Err(Error::Busy)
    ));
    assert!(selected.load().unwrap().digest.is_none());
    drop(lock);
    selected
        .compare_and_publish(None, &prefix(&native, 2))
        .unwrap();
    let foreign = NativeFinalityFixture::new_with_explicit_parameters();
    assert!(matches!(
        selected.compare_and_publish(
            selected.load().unwrap().digest.as_ref(),
            &prefix(&foreign, 2)
        ),
        Err(Error::Binding)
    ));
}

#[test]
fn pending_finality_prefix_retains_authenticated_genesis_without_granting_a_phase_pin() {
    let root = tempfile::tempdir().unwrap();
    let native = NativeFinalityFixture::start("pending-finality-genesis");
    let owner = Arc::new(
        FilePrivateSettlementFinality::open(
            root.path().join("finality"),
            &[29; 32],
            2 * 1024 * 1024,
        )
        .unwrap(),
    );
    let selected = binding(&owner, &native);
    let mut record = prefix(&native, 2);
    assert_eq!(record.prefix.as_ref().unwrap().checkpoint.height(), 1);
    let digest = selected.compare_and_publish(None, &record).unwrap();
    assert_eq!(
        selected
            .load()
            .unwrap()
            .record
            .prefix
            .unwrap()
            .checkpoint
            .height(),
        1
    );
    record.register = Some(PendingFinalityPin {
        identity: identity(30),
        checkpoint: native.checkpoint(),
    });
    assert!(matches!(
        selected.compare_and_publish(Some(&digest), &record),
        Err(Error::Binding)
    ));
    assert_eq!(selected.load().unwrap().digest, Some(digest));
}
