//! Native retained-payload custody, bounds and replay tests.

use super::*;
use crate::{CarChunk, CarStreamingWriter, CarWriter, ChunkProfile, FilePlan};
use std::fs;

#[test]
fn prepared_reader_preserves_multifile_car_and_supports_exact_random_access() {
    let directory = tempfile::tempdir().unwrap();
    let tree = directory.path().join("tree");
    fs::create_dir(&tree).unwrap();
    fs::write(tree.join("a.txt"), b"first file").unwrap();
    fs::write(tree.join("empty.txt"), []).unwrap();
    fs::write(tree.join("index.html"), b"<h1>prepared</h1>").unwrap();
    let (plan, bytes) = CarBuildPlan::from_directory(&tree).unwrap();
    let path = directory.path().join("payload.bin");
    fs::write(&path, &bytes).unwrap();
    let mut source = PreparedCarInput::open(&path, plan.clone()).unwrap();
    assert_eq!(source.plan, plan);
    let mut expected = Vec::new();
    CarWriter::new(&plan, &bytes)
        .unwrap()
        .write_to(&mut expected)
        .unwrap();
    let mut actual = Vec::new();
    CarStreamingWriter::new(&source.plan)
        .write_from_reader(&mut source.payload, &mut actual)
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        source.payload.seek(SeekFrom::End(-4)).unwrap(),
        bytes.len() as u64 - 4
    );
    let mut tail = [0; 4];
    source.payload.read_exact(&mut tail).unwrap();
    assert_eq!(tail, bytes[bytes.len() - 4..]);
    source.payload.rewind().unwrap();
    assert_eq!(source.payload.seek(SeekFrom::Current(3)).unwrap(), 3);
    assert!(source.payload.seek(SeekFrom::Current(-4)).is_err());
    assert!(source.payload.seek(SeekFrom::End(1)).is_err());
    assert!(source.payload.seek(SeekFrom::Start(u64::MAX)).is_err());
    assert_eq!(source.payload.stream_position().unwrap(), 3);
}

#[test]
fn prepared_reader_rejects_invalid_plan_and_inexact_length() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("payload.bin");
    let bytes = b"exact bytes";
    fs::write(&path, bytes).unwrap();
    let plan = CarBuildPlan::single_file(bytes).unwrap();
    let mut invalid = plan.clone();
    invalid.chunks[0].offset = 1;
    assert!(PreparedCarInput::open(&path, invalid).is_err());
    fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(PreparedCarInput::open(&path, plan.clone()).is_err());
    fs::write(&path, [bytes.as_slice(), b"extra"].concat()).unwrap();
    assert!(PreparedCarInput::open(&path, plan.clone()).is_err());
    let mut oversized = plan;
    oversized.content_length = MAX_STREAMING_INPUT_BYTES + 1;
    // Size rejection happens before even opening this deliberately nonexistent source.
    assert_eq!(
        PreparedCarInput::open(&directory.path().join("missing"), oversized)
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[cfg(unix)]
#[test]
fn prepared_reader_rejects_mutation_at_read_seek_and_eof() {
    for operation in 0..3 {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("payload.bin");
        let bytes = b"unchanged";
        fs::write(&path, bytes).unwrap();
        let plan = CarBuildPlan::single_file(bytes).unwrap();
        let mut source = PreparedCarInput::open(&path, plan).unwrap();
        if operation == 2 {
            source.payload.seek(SeekFrom::End(0)).unwrap();
        }
        fs::write(&path, b"modified!").unwrap();
        assert!(source.payload.revalidate().is_err());
        if operation == 1 {
            assert!(source.payload.rewind().is_err());
        } else {
            assert!(source.payload.read(&mut [0; 1]).is_err());
        }
    }
}

#[cfg(windows)]
#[test]
fn prepared_reader_denies_writes_while_retaining_the_original() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("payload.bin");
    let bytes = b"unchanged";
    fs::write(&path, bytes).unwrap();
    let plan = CarBuildPlan::single_file(bytes).unwrap();
    let mut input = PreparedCarInput::open(&path, plan).unwrap();
    // Native Windows custody denies concurrent writes instead of observing them afterwards.
    assert!(fs::write(&path, b"modified!").is_err());
    input.payload.revalidate().unwrap();
    let mut observed = Vec::new();
    input.payload.read_to_end(&mut observed).unwrap();
    assert_eq!(observed, bytes);
}

#[test]
fn sparse_payload_above_eager_limit_is_opened_without_payload_allocation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("payload.bin");
    let length = 513 * 1024 * 1024;
    fs::File::create(&path).unwrap().set_len(length).unwrap();
    let chunk_size = ChunkProfile::DEFAULT.max_size;
    let zero_digest = *blake3::hash(&vec![0; chunk_size]).as_bytes();
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < length {
        let size = (length - offset).min(chunk_size as u64) as u32;
        chunks.push(CarChunk {
            offset,
            length: size,
            digest: zero_digest,
        });
        offset += u64::from(size);
    }
    // Acquisition checks custody and geometry, never claims to authenticate this digest.
    let plan = CarBuildPlan {
        chunk_profile: ChunkProfile::DEFAULT,
        payload_digest: blake3::hash(b"unverified sparse input"),
        content_length: length,
        files: vec![FilePlan {
            path: vec![],
            first_chunk: 0,
            chunk_count: chunks.len(),
            size: length,
        }],
        chunks,
    };
    let mut input = PreparedCarInput::open(&path, plan).unwrap();
    assert_eq!(input.payload.seek(SeekFrom::End(-1)).unwrap(), length - 1);
    let mut byte = [1];
    input.payload.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [0]);
    assert_eq!(input.payload.read(&mut byte).unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn prepared_reader_rejects_links_and_replacement_of_retained_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("payload.bin");
    let link = directory.path().join("link.bin");
    let bytes = b"original";
    fs::write(&path, bytes).unwrap();
    let plan = CarBuildPlan::single_file(bytes).unwrap();
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(PreparedCarInput::open(&link, plan.clone()).is_err());
    fs::remove_file(&link).unwrap();
    fs::hard_link(&path, &link).unwrap();
    assert!(PreparedCarInput::open(&path, plan.clone()).is_err());
    fs::remove_file(&link).unwrap();
    let mut input = PreparedCarInput::open(&path, plan).unwrap();
    fs::rename(&path, directory.path().join("old.bin")).unwrap();
    fs::write(&path, bytes).unwrap();
    assert!(input.payload.read(&mut [0; 1]).is_err());
}
