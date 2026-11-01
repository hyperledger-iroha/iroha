//! Native command replay and destination binding over the real streaming writer.

use super::*;

#[test]
fn native_streaming_replay_retains_guard_and_restores_later_write_position() {
    let temporary = tempfile::tempdir().unwrap();
    let output = temporary.path().join("payload");
    let shared = Arc::new(Mutex::new(StreamingWriter::create(&output).unwrap()));
    {
        let mut guard = shared.lock().unwrap();
        guard.write_chunk(0, b"first").unwrap();
        guard.flush().unwrap();
    }
    {
        let guard = shared.lock().unwrap();
        let mut reader = guard.reader().unwrap();
        let mut prefix = [0; 2];
        reader.read_exact(&mut prefix).unwrap();
        assert_eq!(&prefix, b"fi");
        assert!(shared.try_lock().is_err());
    }
    let mut writer = Arc::try_unwrap(shared).ok().unwrap().into_inner().unwrap();
    writer.write_chunk(1, b" second").unwrap();
    writer.flush().unwrap();
    let mut replay = Vec::new();
    writer.reader().unwrap().read_to_end(&mut replay).unwrap();
    assert_eq!(replay, b"first second");
    assert_eq!(writer.total_written(), 12);
    assert_eq!(writer.current_digest(), *blake3::hash(&replay).as_bytes());
    writer.publish(&output).unwrap();
    assert_eq!(fs::read(output).unwrap(), replay);
}

#[test]
fn native_output_spool_refuses_changed_destination_and_keeps_prior_bytes() {
    let temporary = tempfile::tempdir().unwrap();
    let output = temporary.path().join("payload");
    let other = temporary.path().join("other");
    fs::write(&other, b"prior").unwrap();
    let mut spool = private_output_spool(&output).unwrap();
    let staged = spool.path().to_owned();
    spool.write_all(b"complete but differently bound").unwrap();
    assert!(publish_output_spool(spool, &other).is_err());
    assert!(!staged.exists());
    assert!(!output.exists());
    assert_eq!(fs::read(other).unwrap(), b"prior");
}
