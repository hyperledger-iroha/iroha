//! Retained journal and trace publication contracts exercised on every test host.

use super::*;
use std::io::Write as _;

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().canonicalize().unwrap().join("trace.json");
    (temporary, path)
}

fn journal(path: &Path, bytes: &[u8]) -> RetainedLoadFile {
    let output = JournalOutput::create(path, 1024).unwrap();
    let mut writer = output.writer_file().unwrap();
    writer.write_all(bytes).unwrap();
    writer.sync_all().unwrap();
    drop(writer);
    output
        .seal(RawFileIdentity {
            raw_sha256: Sha256::digest(bytes).into(),
            byte_length: bytes.len() as u64,
        })
        .unwrap()
}

#[test]
fn journal_and_trace_retain_exact_bytes_through_pair_readback() {
    let (_temporary, path) = fixture();
    let journal_path = path.with_file_name("journal");
    let journal = journal(&journal_path, b"journal\n");
    let mut trace = TraceOutput::create(&path, 1024).unwrap();
    trace.file_mut().write_all(b"trace\n").unwrap();
    let trace = trace.publish().unwrap();
    let (left, right) = journal.pair_identity(&trace).unwrap();
    assert_eq!(
        left.raw_sha256,
        <[u8; 32]>::from(Sha256::digest(b"journal\n"))
    );
    assert_eq!(
        right.raw_sha256,
        <[u8; 32]>::from(Sha256::digest(b"trace\n"))
    );
    assert_eq!((left.byte_length, right.byte_length), (8, 6));
    assert!(!path.with_file_name("trace.json.collecting").exists());
    assert!(JournalOutput::create(&journal_path, 1024).is_err());
}

#[test]
fn output_bounds_and_path_checks_refuse_before_publication() {
    let (_temporary, path) = fixture();
    for maximum in [0, super::super::super::MAX_FILE_BYTES + 1] {
        assert!(TraceOutput::create(&path, maximum).is_err());
        assert!(JournalOutput::create(&path, maximum).is_err());
    }
    let parent = path.parent().unwrap().as_os_str();
    let mut dot = parent.to_owned();
    dot.push(format!("{0}.{0}trace", std::path::MAIN_SEPARATOR));
    let mut traversal = parent.to_owned();
    traversal.push(format!("{0}..{0}trace", std::path::MAIN_SEPARATOR));
    for invalid in [
        PathBuf::from("relative"),
        PathBuf::from(dot),
        PathBuf::from(traversal),
    ] {
        assert!(TraceOutput::create(&invalid, 1024).is_err());
        assert!(JournalOutput::create(&invalid, 1024).is_err());
    }
    let mut trace = TraceOutput::create(&path, 3).unwrap();
    trace.file_mut().write_all(b"abc").unwrap();
    assert!(trace.file_mut().write_all(b"d").is_err());
    assert_eq!(trace.publish().unwrap().identity().unwrap().byte_length, 3);

    let journal_path = path.with_file_name("journal");
    let output = JournalOutput::create(&journal_path, 2).unwrap();
    let mut writer = output.writer_file().unwrap();
    writer.write_all(b"abc").unwrap();
    drop(writer);
    assert!(
        output
            .seal(RawFileIdentity {
                raw_sha256: Sha256::digest(b"abc").into(),
                byte_length: 3
            })
            .is_err()
    );
}

#[test]
fn no_replace_publication_preserves_racer_and_partial_trace() {
    let (_temporary, path) = fixture();
    let mut trace = TraceOutput::create(&path, 1024).unwrap();
    trace.file_mut().write_all(b"ours").unwrap();
    std::fs::write(&path, b"racer").unwrap();
    assert!(trace.publish().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"racer");
    assert_eq!(
        std::fs::read(path.with_file_name("trace.json.collecting")).unwrap(),
        b"ours"
    );
}

#[test]
fn journal_seal_rejects_wrong_writer_digest() {
    let (_temporary, path) = fixture();
    let output = JournalOutput::create(&path, 1024).unwrap();
    let mut writer = output.writer_file().unwrap();
    writer.write_all(b"actual").unwrap();
    drop(writer);
    assert!(
        output
            .seal(RawFileIdentity {
                raw_sha256: Sha256::digest(b"wrong!").into(),
                byte_length: 6
            })
            .is_err()
    );
}

#[test]
fn completed_outputs_reject_empty_journal_and_trace() {
    let (_temporary, path) = fixture();
    assert!(TraceOutput::create(&path, 1024).unwrap().publish().is_err());
    let journal_path = path.with_file_name("journal");
    let output = JournalOutput::create(&journal_path, 1024).unwrap();
    assert!(
        output
            .seal(RawFileIdentity {
                raw_sha256: Sha256::digest(b"").into(),
                byte_length: 0,
            })
            .is_err()
    );
}

#[test]
fn restored_namespace_after_failure_does_not_unpoison_output() {
    let (_temporary, path) = fixture();
    let mut output = TraceOutput::create(&path, 1024).unwrap();
    output.file_mut().write_all(b"trace").unwrap();
    let retained = output.publish().unwrap();
    let stage = path.with_file_name("trace.json.collecting");
    std::fs::write(&stage, b"replacement").unwrap();
    assert!(retained.identity().is_err());
    std::fs::remove_file(&stage).unwrap();
    assert!(retained.identity().is_err());
}

#[test]
fn complete_pair_detects_midpoint_namespace_change_and_poisons_both() {
    let (_temporary, path) = fixture();
    let first = journal(&path, b"first");
    let second = journal(&path.with_file_name("second"), b"second");
    let extra = path.with_file_name("extra");
    assert!(
        first
            .pair_identity_with_midpoint(&second, || {
                std::fs::write(&extra, b"changed")?;
                Ok(())
            })
            .is_err()
    );
    std::fs::remove_file(&extra).unwrap();
    assert!(first.identity().is_err());
    assert!(second.identity().is_err());
}

#[cfg(windows)]
#[test]
fn native_windows_handles_block_ancestor_and_leaf_replacement() {
    let (_temporary, path) = fixture();
    let output = TraceOutput::create(&path, 1024).unwrap();
    let stage = path.with_file_name("trace.json.collecting");
    assert!(std::fs::rename(&stage, path.with_file_name("moved")).is_err());
    assert!(
        std::fs::rename(
            path.parent().unwrap(),
            path.parent().unwrap().with_extension("moved")
        )
        .is_err()
    );
    drop(output);
}
