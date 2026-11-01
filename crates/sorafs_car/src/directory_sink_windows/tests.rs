//! Genuine Windows directory publication, original-writer and refusal controls.

use super::*;
use iroha_fs::PublishMode;
use std::process::Command;

fn fixture() -> (tempfile::TempDir, PrivateDirectory, PathBuf) {
    let temporary = tempfile::tempdir().expect("native fixture root");
    let parent = PrivateDirectory::open_or_create(temporary.path().join("private"))
        .expect("canonical private parent");
    let root = parent.path().join("chunks");
    (temporary, parent, root)
}

fn prepared(root: &Path, bytes: &[u8]) -> (DirectoryChunkSink, CarBuildPlan) {
    let plan = CarBuildPlan::single_file(bytes).expect("bounded plan");
    let mut sink = DirectoryChunkSink::new(root);
    sink.prepare(&plan).expect("prepare native sink");
    (sink, plan)
}

fn staging_path(sink: &DirectoryChunkSink) -> PathBuf {
    sink.native
        .staging()
        .expect("original staging")
        .path()
        .to_path_buf()
}

fn grant(path: &Path, sid_access: &str) {
    let output = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", sid_access])
        .output()
        .expect("native ACL tool");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn junction(path: &Path, target: &Path) {
    let output = Command::new("cmd.exe")
        .current_dir(path.parent().expect("native junction parent"))
        .args(["/D", "/C", "mklink", "/J"])
        .arg(path)
        .arg(target)
        .output()
        .expect("native junction creation");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_complete_full_batch_and_tail_preserve_exact_bytes_and_identity() {
    let (_temporary, parent, root) = fixture();
    let payload: Vec<u8> = (0..DIRECTORY_SYNC_BATCH_CHUNKS + 1)
        .map(|index| u8::try_from(index).unwrap())
        .collect();
    let plan = CarBuildPlan::single_file_with_profile(
        &payload,
        ChunkProfile {
            min_size: 1,
            target_size: 1,
            max_size: 1,
            break_mask: 1,
        },
    )
    .unwrap();
    assert_eq!(plan.chunks.len(), 65);
    let mut sink = DirectoryChunkSink::new(&root);
    sink.prepare(&plan).unwrap();
    let staging = staging_path(&sink);
    let identity = sink.native.staging().unwrap().identity().unwrap();
    for (index, chunk) in plan.chunks.iter().enumerate() {
        sink.write_chunk(index, chunk, &payload[index..index + 1])
            .unwrap();
        let pending = (index + 1) % DIRECTORY_SYNC_BATCH_CHUNKS;
        assert_eq!(sink.native.pending_len, pending);
        assert_eq!(
            sink.native
                .pending
                .iter()
                .filter(|file| file.is_some())
                .count(),
            pending
        );
        assert!(!root.exists(), "even a fully synced batch remains private");
    }
    assert_eq!(sink.native.pending_len, 1);
    let output = sink.finish().unwrap();
    assert_eq!(output.publication, DirectoryPublicationStatus::Durable);
    assert_eq!(output.total_bytes, 65);
    assert_eq!(output.records.len(), 65);
    assert!(!staging.exists());
    let complete = parent.open_child("chunks").unwrap();
    assert_eq!(complete.identity().unwrap(), identity);
    for (index, record) in output.records.iter().enumerate() {
        assert_eq!(record.offset, u64::try_from(index).unwrap());
        assert_eq!(record.length, 1);
        assert_eq!(
            record.digest,
            *blake3::hash(&payload[index..index + 1]).as_bytes()
        );
        assert_eq!(
            fs::read(root.join(&record.file_name)).unwrap(),
            [payload[index]]
        );
    }
}

#[test]
fn native_order_metadata_length_digest_and_incomplete_writes_refuse() {
    let (_temporary, _parent, root) = fixture();
    let bytes = b"actual native chunk";
    let (mut sink, plan) = prepared(&root, bytes);
    assert!(matches!(
        sink.write_chunk(1, &plan.chunks[0], bytes),
        Err(ChunkStoreError::SinkChunkOrder {
            expected: 0,
            actual: 1
        })
    ));
    let mut metadata = plan.chunks[0].clone();
    metadata.offset += 1;
    assert!(matches!(
        sink.write_chunk(0, &metadata, bytes),
        Err(ChunkStoreError::SinkChunkMetadataMismatch { .. })
    ));
    assert!(matches!(
        sink.write_chunk(0, &plan.chunks[0], &bytes[..bytes.len() - 1]),
        Err(ChunkStoreError::SinkChunkLengthMismatch { .. })
    ));
    let mut wrong = bytes.to_vec();
    wrong[0] ^= 1;
    assert!(matches!(
        sink.write_chunk(0, &plan.chunks[0], &wrong),
        Err(ChunkStoreError::SinkChunkDigestMismatch { .. })
    ));
    assert_eq!(sink.native.records.len(), 0);
    sink.write_chunk(0, &plan.chunks[0], bytes).unwrap();
    sink.finish().unwrap();
    let incomplete_root = root.with_file_name("incomplete");
    let (incomplete, _) = prepared(&incomplete_root, bytes);
    let staging = staging_path(&incomplete);
    assert!(matches!(
        incomplete.finish(),
        Err(ChunkStoreError::SinkIncomplete { .. })
    ));
    assert!(!incomplete_root.exists());
    assert!(!staging.exists());
}

#[test]
fn native_prepare_refuses_inherited_parent_without_repair() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("chunks");
    let mut sink = DirectoryChunkSink::new(&root);
    assert!(
        sink.prepare(&CarBuildPlan::single_file(b"payload").unwrap())
            .is_err()
    );
    assert!(!root.exists());
    assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
    assert!(PrivateDirectory::open(temporary.path()).is_err());
}

#[test]
fn native_existing_and_late_destinations_never_clobber() {
    for late in [false, true] {
        let (_temporary, parent, root) = fixture();
        let payload = b"source";
        let plan = CarBuildPlan::single_file(payload).unwrap();
        let mut sink = DirectoryChunkSink::new(&root);
        if late {
            sink.prepare(&plan).unwrap();
            sink.write_chunk(0, &plan.chunks[0], payload).unwrap();
        }
        let competitor = parent.create_child("chunks").unwrap();
        competitor
            .write_atomic("sentinel", b"competitor", PublishMode::CreateNew)
            .unwrap();
        let identity = competitor.identity().unwrap();
        if late {
            assert!(sink.finish().is_err());
        } else {
            assert!(sink.prepare(&plan).is_err());
        }
        assert_eq!(competitor.identity().unwrap(), identity);
        assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"competitor");
    }
}

#[test]
fn native_original_pending_writer_excludes_write_delete_and_rename() {
    let (_temporary, _parent, root) = fixture();
    let bytes = b"held original";
    let (mut sink, plan) = prepared(&root, bytes);
    sink.write_chunk(0, &plan.chunks[0], bytes).unwrap();
    let stage = staging_path(&sink);
    let chunk = stage.join("chunk_00000.bin");
    assert!(fs::OpenOptions::new().write(true).open(&chunk).is_err());
    assert!(fs::remove_file(&chunk).is_err());
    assert!(fs::rename(&chunk, stage.join("moved")).is_err());
    assert!(fs::rename(&stage, stage.with_file_name("displaced")).is_err());
    sink.finish().unwrap();
    assert_eq!(fs::read(root.join("chunk_00000.bin")).unwrap(), bytes);
}

#[test]
fn native_readback_refuses_live_competing_writer() {
    let (_temporary, _parent, root) = fixture();
    let bytes = b"held competing writer";
    let (mut sink, plan) = prepared(&root, bytes);
    sink.write_chunk(0, &plan.chunks[0], bytes).unwrap();
    sink.native.flush().unwrap();
    let writer = fs::OpenOptions::new()
        .write(true)
        .open(staging_path(&sink).join("chunk_00000.bin"))
        .unwrap();
    assert!(sink.finish().is_err());
    assert!(!root.exists());
    drop(writer);
}

#[test]
fn native_readback_refuses_tampered_bytes_and_lengths() {
    for wrong in [&b"changed"[..], &b"short"[..]] {
        let (_temporary, _parent, root) = fixture();
        let (mut sink, plan) = prepared(&root, b"payload");
        sink.write_chunk(0, &plan.chunks[0], b"payload").unwrap();
        sink.native.flush().unwrap();
        fs::write(staging_path(&sink).join("chunk_00000.bin"), wrong).unwrap();
        let error = sink.finish().unwrap_err();
        if wrong.len() == 7 {
            assert!(matches!(
                error,
                ChunkStoreError::SinkChunkDigestMismatch { .. }
            ));
        } else {
            assert!(matches!(
                error,
                ChunkStoreError::SinkChunkLengthMismatch { .. }
            ));
        }
        assert!(!root.exists());
    }
}

#[test]
fn native_inventory_refuses_extra_entries_without_publishing() {
    let (_temporary, _parent, root) = fixture();
    let (mut sink, plan) = prepared(&root, b"payload");
    sink.write_chunk(0, &plan.chunks[0], b"payload").unwrap();
    sink.native.flush().unwrap();
    sink.native
        .staging()
        .unwrap()
        .write_atomic("unexpected", b"extra", PublishMode::CreateNew)
        .unwrap();
    assert!(matches!(
        sink.finish(),
        Err(ChunkStoreError::SinkIncomplete {
            expected_chunks: 1,
            actual_chunks: 2,
            ..
        })
    ));
    assert!(!root.exists());
}

#[test]
fn native_readback_refuses_hardlinks_and_preserves_external_original() {
    let (_temporary, parent, root) = fixture();
    let (mut sink, plan) = prepared(&root, b"payload");
    sink.write_chunk(0, &plan.chunks[0], b"payload").unwrap();
    sink.native.flush().unwrap();
    let outside = parent.path().join("outside.bin");
    fs::hard_link(staging_path(&sink).join("chunk_00000.bin"), &outside).unwrap();
    assert!(sink.finish().is_err());
    assert!(!root.exists());
    assert_eq!(fs::read(&outside).unwrap(), b"payload");
}

#[test]
fn native_readback_refuses_junction_and_cleanup_does_not_follow_it() {
    let (_temporary, parent, root) = fixture();
    let outside = parent.create_child("outside").unwrap();
    outside
        .write_atomic("sentinel", b"untouched", PublishMode::CreateNew)
        .unwrap();
    let (mut sink, plan) = prepared(&root, b"payload");
    sink.write_chunk(0, &plan.chunks[0], b"payload").unwrap();
    sink.native.flush().unwrap();
    let stage = staging_path(&sink);
    let chunk = stage.join("chunk_00000.bin");
    fs::remove_file(&chunk).unwrap();
    junction(&chunk, outside.path());
    assert!(sink.finish().is_err());
    assert!(!root.exists());
    assert_eq!(
        outside.read("sentinel", 9).unwrap().as_slice(),
        b"untouched"
    );
    // Delete only the exact test-created junction after sink refusal, never its referent.
    if chunk.exists() {
        fs::remove_dir(&chunk).unwrap();
    }
}

#[test]
fn native_private_acl_drift_refuses_before_publication() {
    for change_parent in [false, true] {
        let (_temporary, _parent, root) = fixture();
        let (mut sink, plan) = prepared(&root, b"payload");
        sink.write_chunk(0, &plan.chunks[0], b"payload").unwrap();
        sink.native.flush().unwrap();
        let stage = staging_path(&sink);
        let changed = if change_parent {
            stage.clone()
        } else {
            stage.join("chunk_00000.bin")
        };
        grant(&changed, "*S-1-1-0:(R)");
        assert!(sink.finish().is_err());
        assert!(!root.exists());
    }
}

#[test]
fn native_prepare_and_chunk_sync_failures_are_permanently_poisoned() {
    let (_temporary, _parent, root) = fixture();
    let plan = CarBuildPlan::single_file(b"payload").unwrap();
    let mut sink = DirectoryChunkSink::new(&root);
    sink.native.fault = Some(Fault::PrepareSync);
    assert!(sink.prepare(&plan).is_err());
    let stage = staging_path(&sink);
    assert!(sink.native.poisoned);
    sink.native.fault = None;
    assert!(sink.write_chunk(0, &plan.chunks[0], b"payload").is_err());
    assert!(sink.native.flush().is_err());
    assert!(sink.prepare(&plan).is_err());
    assert!(sink.finish().is_err());
    assert!(!root.exists());
    assert!(!stage.exists());
    for full in [false, true] {
        let (_temporary, _parent, root) = fixture();
        let payload = vec![7_u8; if full { 65 } else { 1 }];
        let plan = CarBuildPlan::single_file_with_profile(
            &payload,
            ChunkProfile {
                min_size: 1,
                target_size: 1,
                max_size: 1,
                break_mask: 1,
            },
        )
        .unwrap();
        let mut sink = DirectoryChunkSink::new(&root);
        sink.prepare(&plan).unwrap();
        sink.native.fault = Some(Fault::Flush(if full { 63 } else { 0 }));
        let stage = staging_path(&sink);
        if full {
            for index in 0..63 {
                sink.write_chunk(index, &plan.chunks[index], &payload[index..index + 1])
                    .unwrap();
            }
            assert!(
                sink.write_chunk(63, &plan.chunks[63], &payload[63..64])
                    .is_err()
            );
            assert_eq!(sink.native.pending_len, 64);
        } else {
            sink.write_chunk(0, &plan.chunks[0], &payload).unwrap();
            assert!(sink.native.flush().is_err());
            assert_eq!(sink.native.pending_len, 1);
        }
        assert!(sink.native.poisoned);
        sink.native.fault = None;
        assert!(sink.native.flush().is_err());
        assert!(sink.finish().is_err());
        assert!(!root.exists());
        assert!(!stage.exists());
    }
}

#[test]
fn native_post_rename_failures_report_visible_uncertain_publication() {
    for step in [
        DirectoryMoveStep::Published,
        DirectoryMoveStep::SyncDestination,
        DirectoryMoveStep::SyncSource,
        DirectoryMoveStep::Validate,
    ] {
        let (_temporary, _parent, root) = fixture();
        let (mut sink, plan) = prepared(&root, b"published payload");
        sink.write_chunk(0, &plan.chunks[0], b"published payload")
            .unwrap();
        let stage = staging_path(&sink);
        sink.native.fault = Some(Fault::Move(step));
        let output = sink.finish().unwrap();
        assert_eq!(
            output.publication,
            DirectoryPublicationStatus::PublishedButDurabilityUncertain
        );
        assert!(!stage.exists());
        assert_eq!(
            fs::read(root.join("chunk_00000.bin")).unwrap(),
            b"published payload"
        );
        let mut retry = DirectoryChunkSink::new(&root);
        assert!(retry.prepare(&plan).is_err());
        assert_eq!(
            fs::read(root.join("chunk_00000.bin")).unwrap(),
            b"published payload"
        );
    }
}

#[test]
fn native_pre_rename_move_error_preserves_unowned_staging_for_recovery() {
    let (_temporary, parent, root) = fixture();
    let (mut sink, plan) = prepared(&root, b"retained evidence");
    sink.write_chunk(0, &plan.chunks[0], b"retained evidence")
        .unwrap();
    let stage = staging_path(&sink);
    sink.native.fault = Some(Fault::Move(DirectoryMoveStep::Rename));
    assert!(sink.finish().is_err());
    assert!(!root.exists());
    assert_eq!(
        fs::read(stage.join("chunk_00000.bin")).unwrap(),
        b"retained evidence"
    );
    // The failed move consumed the original capability. Only this test's explicit
    // recovery readmission may clean it; sink Drop must never use a saved pathname.
    let recovered = parent.open_child(stage.file_name().unwrap()).unwrap();
    recovered.clear_contents_preserving(&[]).unwrap();
    recovered.remove_empty().unwrap();
}

#[test]
fn native_drop_cleans_only_still_retained_unpublished_staging() {
    let (_temporary, parent, root) = fixture();
    let unrelated = parent.create_child("unrelated").unwrap();
    unrelated
        .write_atomic("sentinel", b"keep", PublishMode::CreateNew)
        .unwrap();
    let (mut sink, plan) = prepared(&root, b"discard");
    sink.write_chunk(0, &plan.chunks[0], b"discard").unwrap();
    let stage = staging_path(&sink);
    drop(sink);
    assert!(!root.exists());
    assert!(!stage.exists());
    assert_eq!(unrelated.read("sentinel", 4).unwrap().as_slice(), b"keep");
}
