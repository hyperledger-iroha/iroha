//! Real private-file custody over genuine native certificates with synthetic execution.
//! Slot identities here exercise storage, not the observer's separate monetary verification.

use super::*;
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;

const TEST_CAP: usize = 4 * 1024 * 1024;

fn owner(path: &Path) -> Arc<FileNativePaymentFinality> {
    Arc::new(FileNativePaymentFinality::open(path, &[21; 32], TEST_CAP).unwrap())
}
fn bind(
    owner: &Arc<FileNativePaymentFinality>,
    native: &NativeFinalityFixture,
) -> BoundNativePaymentFinality {
    owner
        .bind(
            WorkflowId::from_bytes([22; 32]),
            [23; 32],
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
            lookup: FinalityLookup::Carrier {
                identity: identity(24),
                height: target,
            },
            checkpoint: native.checkpoint(),
        }),
        ..Default::default()
    }
}
fn stored_bytes(owner: &FileNativePaymentFinality) -> Vec<u8> {
    std::fs::read(owner.directory.path().join(RECORD_NAME)).unwrap()
}

#[test]
fn original_pending_owner_opens_without_files_and_refuses_unbounded_or_zero_key() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let original = owner(&path);
    let native = NativeFinalityFixture::new();
    let selected = bind(&original, &native);
    assert!(selected.load().unwrap().digest.is_none());
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    for cap in [0, MAX_NATIVE_PAYMENT_FINALITY_BYTES + 1] {
        assert!(matches!(
            FileNativePaymentFinality::open(&path, &[21; 32], cap),
            Err(Error::Capacity)
        ));
    }
    assert!(matches!(
        FileNativePaymentFinality::open(&path, &[0; 32], TEST_CAP),
        Err(Error::Binding)
    ));
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
}

#[test]
fn three_slots_reopen_exactly_with_replay_locator_and_preserve_original_success() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let original = owner(&path);
    let mut native = NativeFinalityFixture::new();
    let selected = bind(&original, &native);
    let mut record = prefix(&native, 70);
    let first = selected.compare_and_publish(None, &record).unwrap();
    let old = native.checkpoint();
    let replay = BusinessLocator {
        height: 2,
        block_hash: [29; 32],
    };
    native.certify(native.block_with_submitted_work(native.next_header()));
    record.carrier = Some(PendingCarrierPin {
        identity: identity(30),
        checkpoint: native.checkpoint(),
        replay: Some(replay),
    });
    record.business = Some(PendingBusinessPin {
        identity: identity(31),
        checkpoint: old.clone(),
    });
    // A corrected earlier lookup can replace the pending cursor. The separate successful
    // business pin remains original even when the exact selected carrier is different.
    record.prefix = Some(PendingFinalityPrefix {
        lookup: FinalityLookup::Business(replay),
        checkpoint: old,
    });
    let second = selected.compare_and_publish(Some(&first), &record).unwrap();
    assert_ne!(first, second);
    assert!(matches!(
        selected.compare_and_publish(Some(&first), &record),
        Err(Error::Conflict)
    ));
    let ciphertext = stored_bytes(&original);
    assert_ne!(ciphertext, *encode_record(&record, TEST_CAP).unwrap());
    drop(selected);
    drop(original);
    let reopened = owner(&path);
    let selected = bind(&reopened, &native);
    let restored = selected.load().unwrap();
    assert_eq!(restored.digest, Some(second));
    assert_eq!(
        restored.record.carrier.as_ref().unwrap().replay,
        Some(replay)
    );
    assert_eq!(
        *encode_record(&restored.record, TEST_CAP).unwrap(),
        *encode_record(&record, TEST_CAP).unwrap()
    );
    assert_eq!(stored_bytes(&reopened), ciphertext);
    for changed in [
        None,
        Some(PendingBusinessPin {
            identity: identity(32),
            checkpoint: native.checkpoint(),
        }),
    ] {
        let mut replacement = record.clone();
        replacement.business = changed;
        assert!(matches!(
            selected.compare_and_publish(Some(&second), &replacement),
            Err(Error::Binding)
        ));
        assert_eq!(stored_bytes(&reopened), ciphertext);
    }
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 2);
}

#[test]
fn ciphertext_binds_original_key_workflow_request_root_and_initial_epoch() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let original = owner(&path);
    let native = NativeFinalityFixture::new();
    let selected = bind(&original, &native);
    let record = prefix(&native, 70);
    selected.compare_and_publish(None, &record).unwrap();
    let ciphertext = stored_bytes(&original);
    let foreign = NativeFinalityFixture::start("foreign selected native payment");
    assert!(matches!(
        bind(&original, &foreign).load(),
        Err(Error::Binding)
    ));
    let wrong_key = Arc::new(FileNativePaymentFinality::open(&path, &[33; 32], TEST_CAP).unwrap());
    assert!(matches!(
        bind(&wrong_key, &native).load(),
        Err(Error::Binding)
    ));
    for field in 0..5 {
        let mut changed = selected.clone();
        match field {
            0 => changed.binding.workflow[0] ^= 1,
            1 => changed.binding.request[0] ^= 1,
            2 => changed.binding.chain.push('!'),
            3 => changed.binding.instance[0] ^= 1,
            4 => changed.binding.initial_epoch[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(matches!(changed.load(), Err(Error::Binding)));
        assert!(matches!(
            changed.compare_and_publish(None, &record),
            Err(Error::Binding | Error::Conflict)
        ));
    }
    assert_eq!(stored_bytes(&original), ciphertext);
    let mut corrupt = ciphertext;
    *corrupt.last_mut().unwrap() ^= 1;
    original
        .directory
        .write_atomic(RECORD_NAME, &corrupt, PublishMode::Replace)
        .unwrap();
    assert!(matches!(selected.load(), Err(Error::Binding)));
    assert!(matches!(
        selected.compare_and_publish(None, &record),
        Err(Error::Binding)
    ));
    assert_eq!(stored_bytes(&original), corrupt);
}

#[test]
fn every_present_native_frame_decodes_even_when_another_pin_could_answer() {
    let root = tempfile::tempdir().unwrap();
    let original = owner(&root.path().join("pending"));
    let native = NativeFinalityFixture::new();
    let selected = bind(&original, &native);
    selected
        .compare_and_publish(None, &prefix(&native, 2))
        .unwrap();
    let malformed = StoredRecord {
        prefix: None,
        carrier: Some(StoredCarrier {
            identity: identity(35),
            checkpoint: vec![0; 8],
            replay: None,
        }),
        business: Some(StoredBusiness {
            identity: identity(36),
            checkpoint: native.checkpoint().encode_canonical().unwrap(),
        }),
    };
    let plaintext = norito::encode_canonical(&malformed).unwrap();
    let encrypted = original
        .cipher
        .encrypt_easy(selected.aad().unwrap().as_slice(), plaintext.as_slice())
        .unwrap();
    original
        .directory
        .write_atomic(RECORD_NAME, &encrypted, PublishMode::Replace)
        .unwrap();
    assert!(matches!(selected.load(), Err(Error::Finality(_))));
    assert!(matches!(
        selected.compare_and_publish(None, &prefix(&native, 2)),
        Err(Error::Finality(_))
    ));
    assert_eq!(stored_bytes(&original), encrypted);
    let wrong_purpose = original
        .cipher
        .encrypt_easy(b"other original custody".as_slice(), plaintext.as_slice())
        .unwrap();
    original
        .directory
        .write_atomic(RECORD_NAME, &wrong_purpose, PublishMode::Replace)
        .unwrap();
    assert!(matches!(selected.load(), Err(Error::Binding)));
}

#[test]
fn cap_and_original_lock_refuse_before_replacement_and_keep_cas_resumable() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let native = NativeFinalityFixture::new();
    let record = prefix(&native, 70);
    let length = encode_record(&record, TEST_CAP).unwrap().len();
    let constrained =
        Arc::new(FileNativePaymentFinality::open(&path, &[21; 32], length - 1).unwrap());
    assert!(matches!(
        bind(&constrained, &native).compare_and_publish(None, &record),
        Err(Error::Capacity)
    ));
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    let exact = Arc::new(FileNativePaymentFinality::open(&path, &[21; 32], length).unwrap());
    let selected = bind(&exact, &native);
    let digest = selected.compare_and_publish(None, &record).unwrap();
    let ciphertext = stored_bytes(&exact);
    assert!(bind(&constrained, &native).load().is_err());
    let mut expanded = record.clone();
    expanded.carrier = Some(PendingCarrierPin {
        identity: identity(37),
        checkpoint: native.checkpoint(),
        replay: None,
    });
    assert!(matches!(
        selected.compare_and_publish(Some(&digest), &expanded),
        Err(Error::Capacity)
    ));
    let lock = exact.directory.open_lock(LOCK_NAME).unwrap();
    lock.try_lock().unwrap();
    assert!(matches!(selected.load(), Err(Error::Busy)));
    assert!(matches!(
        selected.compare_and_publish(Some(&digest), &record),
        Err(Error::Busy)
    ));
    drop(lock);
    assert_eq!(selected.load().unwrap().digest, Some(digest));
    assert_eq!(stored_bytes(&exact), ciphertext);
    // Native publication refusal cannot overwrite the original. Restore the original
    // named file and the same compare-and-publish remains eligible, without a new key.
    std::fs::rename(path.join(RECORD_NAME), path.join("retained-original")).unwrap();
    std::fs::create_dir(path.join(RECORD_NAME)).unwrap();
    assert!(
        selected
            .compare_and_publish(Some(&digest), &record)
            .is_err()
    );
    std::fs::remove_dir(path.join(RECORD_NAME)).unwrap();
    std::fs::rename(path.join("retained-original"), path.join(RECORD_NAME)).unwrap();
    assert_eq!(
        selected
            .compare_and_publish(Some(&digest), &record)
            .unwrap(),
        digest
    );
}

#[test]
fn original_writer_reconciles_only_one_bounded_native_staging_envelope() {
    let root = tempfile::tempdir().unwrap();
    let original = owner(&root.path().join("pending"));
    let native = NativeFinalityFixture::new();
    let selected = bind(&original, &native);
    let digest = selected
        .compare_and_publish(None, &prefix(&native, 70))
        .unwrap();
    let ciphertext = stored_bytes(&original);
    let staged = ".iroha-fs-921-0.tmp";
    for _ in 0..3 {
        original
            .directory
            .write_atomic(staged, b"interrupted ciphertext", PublishMode::CreateNew)
            .unwrap();
        assert_eq!(selected.load().unwrap().digest, Some(digest));
        assert!(!original.directory.path().join(staged).exists());
        assert_eq!(stored_bytes(&original), ciphertext);
    }
    original
        .directory
        .write_atomic(staged, b"interrupted ciphertext", PublishMode::CreateNew)
        .unwrap();
    for unknown in [
        ".iroha-fs-921-1.tmp",
        "unrelated-original",
        ".iroha-fs-invalid.tmp",
    ] {
        original
            .directory
            .write_atomic(unknown, b"unknown", PublishMode::CreateNew)
            .unwrap();
        assert!(selected.load().is_err());
        assert!(
            original.directory.path().join(staged).exists(),
            "full inventory refuses before deleting staging"
        );
        original.directory.remove_private(unknown).unwrap();
    }
    std::fs::OpenOptions::new()
        .write(true)
        .open(original.directory.path().join(staged))
        .unwrap()
        .set_len((TEST_CAP + AEAD_OVERHEAD + 1) as u64)
        .unwrap();
    assert!(selected.load().is_err());
    assert!(original.directory.path().join(staged).exists());
    original.directory.remove_private(staged).unwrap();
    let lock = original.directory.open_lock(LOCK_NAME).unwrap();
    lock.set_len(1).unwrap();
    assert!(matches!(selected.load(), Err(Error::Binding)));
    lock.set_len(0).unwrap();
    assert_eq!(selected.load().unwrap().digest, Some(digest));
    assert_eq!(stored_bytes(&original), ciphertext);
}

#[test]
fn pending_prefix_accepts_genesis_only_as_progress_and_refuses_invalid_pin_shapes() {
    let root = tempfile::tempdir().unwrap();
    let original = owner(&root.path().join("pending"));
    let genesis = NativeFinalityFixture::start("pending original genesis");
    let selected = bind(&original, &genesis);
    let mut record = prefix(&genesis, 2);
    assert_eq!(record.prefix.as_ref().unwrap().checkpoint.height(), 1);
    let digest = selected.compare_and_publish(None, &record).unwrap();
    record.carrier = Some(PendingCarrierPin {
        identity: identity(39),
        checkpoint: genesis.checkpoint(),
        replay: None,
    });
    assert!(matches!(
        selected.compare_and_publish(Some(&digest), &record),
        Err(Error::Binding)
    ));
    record.carrier = None;
    record.business = Some(PendingBusinessPin {
        identity: identity(39),
        checkpoint: genesis.checkpoint(),
    });
    assert!(matches!(
        selected.compare_and_publish(Some(&digest), &record),
        Err(Error::Binding)
    ));
    record.business = None;
    record.prefix.as_mut().unwrap().lookup = FinalityLookup::Business(BusinessLocator {
        height: 1,
        block_hash: [40; 32],
    });
    assert!(matches!(
        selected.compare_and_publish(Some(&digest), &record),
        Err(Error::Binding)
    ));
    assert_eq!(selected.load().unwrap().digest, Some(digest));
}
