//! Crash recovery, canonical lineage, concurrency and filesystem rejection for software checkpoints.
use super::*;

fn checkpoint(root: &Path) -> NativeCheckpointV1 {
    NativeCheckpointV1 {
        handle: "software://sorafs/native-checkpoint".to_owned(),
        qualification: ProviderIngestCheckpointProviderQualificationV1::new(1, [1; 32]),
        root: root.join("authority"),
        maximum: 4096,
        gate: Mutex::new(()),
        lock: OnceLock::new(),
    }
}

fn record(
    previous: Option<&ProviderIngestSealedCheckpointRecordV1>,
    marker: u8,
) -> ProviderIngestSealedCheckpointRecordV1 {
    let mut record = ProviderIngestSealedCheckpointRecordV1 {
        namespace: *b"sorafs.provider.ingest.outbox.v1",
        version: 1,
        checkpoint_sequence: previous.map_or(1, |value| value.checkpoint_sequence + 1),
        predecessor_revision: previous.map(|value| value.revision),
        predecessor_checkpoint_digest: previous.map(|value| value.checkpoint_digest),
        checkpoint_bytes: norito::to_bytes(&marker).unwrap(),
        checkpoint_digest: [0; 32],
        revision: [0; 32],
    };
    record.checkpoint_digest = *blake3::hash(&record.checkpoint_bytes).as_bytes();
    let mut hash = blake3::Hasher::new();
    hash.update(b"sorafs.provider.ingest.sealed-checkpoint.revision.v1\0");
    hash.update(&record.namespace);
    hash.update(&[record.version]);
    hash.update(&record.checkpoint_sequence.to_le_bytes());
    for predecessor in [
        record.predecessor_revision,
        record.predecessor_checkpoint_digest,
    ] {
        if let Some(value) = predecessor {
            hash.update(&[1]);
            hash.update(&value);
        } else {
            hash.update(&[0]);
        }
    }
    hash.update(&record.checkpoint_digest);
    hash.update(&(record.checkpoint_bytes.len() as u64).to_le_bytes());
    hash.update(&record.checkpoint_bytes);
    record.revision = *hash.finalize().as_bytes();
    record.validate(4096).unwrap();
    record
}

#[test]
fn native_checkpoint_cas_survives_restart_and_rejects_stale_or_corrupt_records() {
    let temporary = tempfile::tempdir().unwrap();
    let store = checkpoint(temporary.path());
    assert!(store.load_latest().unwrap().is_none());
    let first = record(None, 1);
    store.compare_and_swap_latest(None, &first).unwrap();
    let second = record(Some(&first), 2);
    assert!(store.compare_and_swap_latest(None, &second).is_err());
    assert_eq!(store.load_latest().unwrap(), Some(first.clone()));
    store
        .compare_and_swap_latest(Some(first.revision), &second)
        .unwrap();
    let other = checkpoint(temporary.path());
    assert!(
        other.load_latest().is_err(),
        "a second process incarnation cannot share authority"
    );
    drop(store);
    assert_eq!(other.load_latest().unwrap(), Some(second.clone()));
    let mut corrupt = record(Some(&second), 3);
    corrupt.checkpoint_bytes[0] ^= 1;
    assert!(
        other
            .compare_and_swap_latest(Some(second.revision), &corrupt)
            .is_err()
    );
    assert_eq!(other.load_latest().unwrap(), Some(second));
}

#[test]
fn native_checkpoint_competing_successors_have_exactly_one_winner() {
    let temporary = tempfile::tempdir().unwrap();
    let store = Arc::new(checkpoint(temporary.path()));
    let first = record(None, 1);
    store.compare_and_swap_latest(None, &first).unwrap();
    let next = [record(Some(&first), 2), record(Some(&first), 3)];
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let handles = next
        .iter()
        .cloned()
        .map(|next| {
            let store = Arc::clone(&store);
            let barrier = Arc::clone(&barrier);
            let revision = first.revision;
            std::thread::spawn(move || {
                barrier.wait();
                store.compare_and_swap_latest(Some(revision), &next)
            })
        })
        .collect::<Vec<_>>();
    let successes = handles
        .into_iter()
        .map(|handle| handle.join().unwrap().is_ok())
        .filter(|success| *success)
        .count();
    assert_eq!(successes, 1);
    assert!(next.contains(&store.load_latest().unwrap().unwrap()));
}

#[test]
fn native_checkpoint_rejects_symlink_and_oversized_payload_before_read_or_write() {
    let temporary = tempfile::tempdir().unwrap();
    let store = checkpoint(temporary.path());
    store.load_latest().unwrap();
    let sentinel = temporary.path().join("untouched");
    fs::write(&sentinel, b"sentinel").unwrap();
    std::os::unix::fs::symlink(&sentinel, store.root.join("checkpoint.pending")).unwrap();
    assert!(
        store
            .compare_and_swap_latest(None, &record(None, 1))
            .is_err()
    );
    assert_eq!(fs::read(&sentinel).unwrap(), b"sentinel");
    std::os::unix::fs::symlink(&sentinel, store.root.join("checkpoint.to")).unwrap();
    assert!(store.load_latest().is_err());
    fs::remove_file(store.root.join("checkpoint.to")).unwrap();
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(store.root.join("checkpoint.to"))
        .unwrap();
    file.set_len(8193).unwrap();
    assert!(store.load_latest().is_err());
}
