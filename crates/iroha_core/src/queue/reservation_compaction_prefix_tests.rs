//! Tests for the production bounded reservation-compaction reader.

use super::{SCRATCH_BYTES, verify_prefix};
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Cursor, Read, Seek as _, SeekFrom, Write as _},
};

fn payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| u8::try_from(index % 251).unwrap())
        .collect()
}

struct Observed<R> {
    inner: R,
    max_chunk: usize,
    returned: usize,
    largest_request: usize,
    calls: usize,
}

impl<R> Observed<R> {
    fn new(inner: R, max_chunk: usize) -> Self {
        Self {
            inner,
            max_chunk,
            returned: 0,
            largest_request: 0,
            calls: 0,
        }
    }
}

impl<R: Read> Read for Observed<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        self.largest_request = self.largest_request.max(bytes.len());
        let take = bytes.len().min(self.max_chunk);
        let count = self.inner.read(&mut bytes[..take])?;
        self.returned += count;
        Ok(count)
    }
}

enum Step {
    Forward(usize),
    Error(io::Error),
}

impl Step {
    fn error(kind: io::ErrorKind) -> Self {
        Self::Error(io::Error::new(kind, "original scripted read error"))
    }
}

struct Scripted {
    input: Cursor<Vec<u8>>,
    steps: VecDeque<Step>,
}

impl Read for Scripted {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self.steps.pop_front() {
            Some(Step::Error(error)) => Err(error),
            Some(Step::Forward(maximum)) => {
                let take = bytes.len().min(maximum);
                self.input.read(&mut bytes[..take])
            }
            None => self.input.read(bytes),
        }
    }
}

#[test]
fn exact_streams_and_partial_reads_cover_every_scratch_boundary() {
    for length in [
        0,
        1,
        SCRATCH_BYTES - 1,
        SCRATCH_BYTES,
        SCRATCH_BYTES + 1,
        SCRATCH_BYTES * 2,
        SCRATCH_BYTES * 2 + 1,
    ] {
        let expected = payload(length);
        for max_chunk in [1, 7, SCRATCH_BYTES] {
            let mut reader = Observed::new(Cursor::new(expected.clone()), max_chunk);
            verify_prefix(&mut reader, &expected, length as u64).unwrap();
            assert_eq!(reader.returned, length);
            assert!(reader.largest_request <= SCRATCH_BYTES);
            assert!(reader.calls > 0, "even an empty prefix must check EOF");
        }
    }
}

#[test]
fn shorter_admitted_prefix_does_not_require_the_rest_of_expected_image() {
    let expected = payload(SCRATCH_BYTES * 2 + 3);
    for length in [0, 1, SCRATCH_BYTES, SCRATCH_BYTES + 1] {
        let mut reader = Observed::new(Cursor::new(expected[..length].to_vec()), SCRATCH_BYTES);
        verify_prefix(&mut reader, &expected, length as u64).unwrap();
        assert_eq!(reader.returned, length);
        assert!(reader.largest_request <= SCRATCH_BYTES);
    }
}

#[test]
fn mismatch_at_first_boundary_and_last_byte_never_returns_partial_success() {
    let expected = payload(SCRATCH_BYTES * 2 + 1);
    for index in [0, SCRATCH_BYTES - 1, SCRATCH_BYTES, expected.len() - 1] {
        let mut actual = expected.clone();
        actual[index] ^= 0x80;
        let mut reader = Observed::new(Cursor::new(actual), SCRATCH_BYTES);
        let error = verify_prefix(&mut reader, &expected, expected.len() as u64).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("not an authenticated prefix"));
        assert_eq!(
            reader.returned,
            expected.len(),
            "retain I/O precedence over mismatch"
        );
        assert!(reader.largest_request <= SCRATCH_BYTES);
    }
}

#[test]
fn oversize_admission_refuses_before_any_read() {
    let expected = payload(3);
    for admitted in [4, u64::MAX] {
        let mut reader = Observed::new(Cursor::new(expected.clone()), SCRATCH_BYTES);
        let error = verify_prefix(&mut reader, &expected, admitted).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(reader.calls, 0);
        assert_eq!(reader.returned, 0);
    }
}

struct GrowingReader {
    expected: Vec<u8>,
    returned: usize,
    largest_request: usize,
}

impl Read for GrowingReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.largest_request = self.largest_request.max(bytes.len());
        assert!(
            bytes.len() <= SCRATCH_BYTES,
            "read request exceeds fixed scratch"
        );
        assert!(
            self.returned + bytes.len() <= self.expected.len() + 1,
            "reader attempted to consume an unbounded growing tail"
        );
        for byte in bytes.iter_mut() {
            *byte = self.expected.get(self.returned).copied().unwrap_or(0xe1);
            self.returned += 1;
        }
        // The source never reports EOF. The helper must stop after the probe.
        Ok(bytes.len())
    }
}

#[test]
fn a_never_ending_growing_source_is_rejected_after_exactly_one_extra_byte() {
    for length in [
        0,
        1,
        SCRATCH_BYTES,
        SCRATCH_BYTES + 1,
        SCRATCH_BYTES * 2 + 1,
    ] {
        let expected = payload(length);
        let mut reader = GrowingReader {
            expected: expected.clone(),
            returned: 0,
            largest_request: 0,
        };
        let error = verify_prefix(&mut reader, &expected, length as u64).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("identity or length changed"));
        assert_eq!(reader.returned, length + 1);
        assert!(reader.largest_request <= SCRATCH_BYTES);
    }
}

#[test]
fn a_short_stream_preserves_unexpected_eof_without_accepting_a_smaller_prefix() {
    let expected = payload(SCRATCH_BYTES * 2 + 1);
    for available in [0, 1, SCRATCH_BYTES - 1, SCRATCH_BYTES, expected.len() - 1] {
        let mut reader = Observed::new(Cursor::new(expected[..available].to_vec()), 17);
        let error = verify_prefix(&mut reader, &expected, expected.len() as u64).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(reader.returned, available);
        assert!(reader.largest_request <= SCRATCH_BYTES);
    }
}

#[test]
fn interruptions_in_body_and_eof_probe_are_retried_without_skipping_bytes() {
    let expected = payload(SCRATCH_BYTES);
    let mut reader = Observed::new(
        Scripted {
            input: Cursor::new(expected.clone()),
            steps: VecDeque::from([
                Step::error(io::ErrorKind::Interrupted),
                Step::Forward(SCRATCH_BYTES),
                Step::error(io::ErrorKind::Interrupted),
                Step::error(io::ErrorKind::Interrupted),
            ]),
        },
        SCRATCH_BYTES,
    );
    verify_prefix(&mut reader, &expected, expected.len() as u64).unwrap();
    assert_eq!(reader.returned, expected.len());
    assert_eq!(reader.calls, 5);
    assert!(reader.inner.steps.is_empty());
}

#[test]
fn eof_probe_error_is_propagated_for_empty_and_nonempty_prefixes() {
    for length in [0, SCRATCH_BYTES] {
        let expected = payload(length);
        let mut steps = VecDeque::new();
        if length != 0 {
            steps.push_back(Step::Forward(length));
        }
        steps.push_back(Step::error(io::ErrorKind::PermissionDenied));
        let mut reader = Observed::new(
            Scripted {
                input: Cursor::new(expected.clone()),
                steps,
            },
            SCRATCH_BYTES,
        );
        let error = verify_prefix(&mut reader, &expected, length as u64).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), "original scripted read error");
        assert_eq!(reader.returned, length);
    }
}

#[test]
fn earlier_invalid_bytes_do_not_hide_a_later_body_io_error() {
    let expected = payload(SCRATCH_BYTES + 1);
    let mut actual = expected.clone();
    actual[0] ^= 0x80;
    let mut reader = Observed::new(
        Scripted {
            input: Cursor::new(actual),
            steps: VecDeque::from([
                Step::Forward(SCRATCH_BYTES),
                Step::error(io::ErrorKind::BrokenPipe),
            ]),
        },
        SCRATCH_BYTES,
    );
    let error = verify_prefix(&mut reader, &expected, expected.len() as u64).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "original scripted read error");
    assert_eq!(reader.returned, SCRATCH_BYTES);
}

#[test]
fn earlier_invalid_bytes_do_not_hide_a_later_probe_io_error() {
    let expected = payload(SCRATCH_BYTES);
    let mut actual = expected.clone();
    actual[0] ^= 0x80;
    let mut reader = Observed::new(
        Scripted {
            input: Cursor::new(actual),
            steps: VecDeque::from([
                Step::Forward(SCRATCH_BYTES),
                Step::error(io::ErrorKind::BrokenPipe),
            ]),
        },
        SCRATCH_BYTES,
    );
    let error = verify_prefix(&mut reader, &expected, expected.len() as u64).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "original scripted read error");
    assert_eq!(reader.returned, expected.len());
}

enum Change {
    Append(Vec<u8>),
    Truncate(u64),
}

struct ChangingFile<'a> {
    original: &'a mut File,
    mutator: File,
    change: Option<Change>,
}

impl Read for ChangingFile<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if let Some(change) = self.change.take() {
            match change {
                Change::Append(extra) => {
                    self.mutator.seek(SeekFrom::End(0))?;
                    self.mutator.write_all(&extra)?;
                }
                Change::Truncate(length) => self.mutator.set_len(length)?,
            }
        }
        self.original.read(bytes)
    }
}

#[test]
fn actual_file_growth_after_length_capture_is_bounded_and_preserved() {
    for admitted in [0, 1, SCRATCH_BYTES + 1] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("compaction.tmp");
        let expected = payload(admitted);
        std::fs::write(&path, &expected).unwrap();
        let mut original = File::open(&path).unwrap();
        let captured_len = original.metadata().unwrap().len();
        let mutator = OpenOptions::new().write(true).open(&path).unwrap();
        let extra = vec![0xab; SCRATCH_BYTES * 3];
        let mut reader = Observed::new(
            ChangingFile {
                original: &mut original,
                mutator,
                change: Some(Change::Append(extra.clone())),
            },
            SCRATCH_BYTES,
        );
        let error = verify_prefix(&mut reader, &expected, captured_len).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(reader.returned, admitted + 1);
        assert!(reader.largest_request <= SCRATCH_BYTES);
        drop(reader);
        let mut evidence = expected;
        evidence.extend_from_slice(&extra);
        assert_eq!(std::fs::read(&path).unwrap(), evidence);
        assert_eq!(original.stream_position().unwrap(), captured_len + 1);
    }
}

#[test]
fn actual_file_truncation_after_length_capture_is_rejected_and_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("compaction.tmp");
    let expected = payload(SCRATCH_BYTES + 1);
    std::fs::write(&path, &expected).unwrap();
    let mut original = File::open(&path).unwrap();
    let captured_len = original.metadata().unwrap().len();
    let mutator = OpenOptions::new().write(true).open(&path).unwrap();
    let mut reader = Observed::new(
        ChangingFile {
            original: &mut original,
            mutator,
            change: Some(Change::Truncate(17)),
        },
        SCRATCH_BYTES,
    );
    let error = verify_prefix(&mut reader, &expected, captured_len).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(reader.returned, 17);
    assert!(reader.largest_request <= SCRATCH_BYTES);
    drop(reader);
    assert_eq!(std::fs::read(&path).unwrap(), expected[..17]);
    assert_eq!(original.stream_position().unwrap(), 17);
}

#[test]
fn borrowed_file_position_is_respected_without_reopen_or_seek() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("compaction.tmp");
    let bytes = payload(SCRATCH_BYTES + 1);
    std::fs::write(&path, &bytes).unwrap();
    let mut original = File::open(&path).unwrap();
    original.seek(SeekFrom::Start(17)).unwrap();
    verify_prefix(&mut original, &bytes[17..], (bytes.len() - 17) as u64).unwrap();
    assert_eq!(original.stream_position().unwrap(), bytes.len() as u64);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
