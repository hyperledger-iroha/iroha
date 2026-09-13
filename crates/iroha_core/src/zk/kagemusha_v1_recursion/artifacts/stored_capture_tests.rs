//! Independent storage-contract tests for original-byte capture, not release authentication.
//!
//! The existing `for_stream_tests` fixture deliberately bypasses release authentication and the
//! byte pattern is not a canonical proving key. These tests use the actual encrypted spool on
//! Unix, but cannot access its private ciphertext/fault hooks across the crypto crate boundary.
//! They make no FD/key-erasure, allocation-failure, full-decoder, RSS or device-security claim.
//! Capture's one live 8 KiB chunk excludes the directory resolver's separate 64 KiB buffer,
//! crypto metadata and all output buffers owned by these test oracles.

use std::{
    fs,
    io::{self, Read as _, Seek as _, SeekFrom},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::super::{KagemushaDirectoryArtifactResolverV1, lower_hex, role_index};
use super::*;

const ROLE: KagemushaArtifactRoleV1 = KagemushaArtifactRoleV1::MintHashClaimPkEq;
const WIDTH: usize = 8192;

#[derive(Default)]
struct Observations {
    opens: AtomicUsize,
    reads: AtomicUsize,
    bytes: AtomicUsize,
    drops: AtomicUsize,
    largest_request: AtomicUsize,
}

#[derive(Clone, Copy)]
enum SourceFault {
    None,
    InterruptedAt(usize),
    ErrorAt(usize),
    PanicAt(usize),
    Overreport,
}

struct StreamingResolver {
    bytes: Arc<[u8]>,
    maximum_read: usize,
    fault: SourceFault,
    observed: Arc<Observations>,
}

struct SourceReader {
    bytes: Arc<[u8]>,
    position: usize,
    maximum_read: usize,
    fault: SourceFault,
    observed: Arc<Observations>,
}

impl KagemushaArtifactByteResolverV1 for StreamingResolver {
    fn resolve_bytes(
        &self,
        _: KagemushaArtifactBindingV1,
    ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        panic!("capture must use the original streaming resolver");
    }

    fn open_reader(
        &self,
        _: KagemushaArtifactBindingV1,
    ) -> Result<Box<dyn io::Read + Send>, KagemushaArtifactErrorV1> {
        self.observed.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(SourceReader {
            bytes: Arc::clone(&self.bytes),
            position: 0,
            maximum_read: self.maximum_read,
            fault: self.fault,
            observed: Arc::clone(&self.observed),
        }))
    }
}

impl io::Read for SourceReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.observed.reads.fetch_add(1, Ordering::SeqCst);
        self.observed
            .largest_request
            .fetch_max(output.len(), Ordering::SeqCst);
        match self.fault {
            SourceFault::InterruptedAt(at) if self.position == at => {
                self.fault = SourceFault::None;
                return Err(io::ErrorKind::Interrupted.into());
            }
            SourceFault::ErrorAt(at) if self.position == at => {
                return Err(io::Error::other("independent source fault"));
            }
            SourceFault::PanicAt(at) if self.position == at => {
                panic!("independent source unwind");
            }
            SourceFault::Overreport => return Ok(output.len() + 1),
            _ => {}
        }
        let mut count = output
            .len()
            .min(self.maximum_read)
            .min(self.bytes.len() - self.position);
        match self.fault {
            SourceFault::InterruptedAt(at)
            | SourceFault::ErrorAt(at)
            | SourceFault::PanicAt(at)
                if at > self.position =>
            {
                count = count.min(at - self.position);
            }
            _ => {}
        }
        output[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
        self.position += count;
        self.observed.bytes.fetch_add(count, Ordering::SeqCst);
        Ok(count)
    }
}

impl Drop for SourceReader {
    fn drop(&mut self) {
        self.observed.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn pattern(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((index * 29 + index / 251 + 17) % 256) as u8)
        .collect()
}

fn binding(role: KagemushaArtifactRoleV1, bytes: &[u8]) -> KagemushaArtifactBindingV1 {
    KagemushaArtifactBindingV1 {
        role,
        sha256: Sha256::digest(bytes).into(),
        byte_len: bytes.len() as u64,
    }
}

fn fixture(
    bytes: Vec<u8>,
    binding: KagemushaArtifactBindingV1,
    maximum_read: usize,
    fault: SourceFault,
) -> (
    KagemushaAuthenticatedArtifactSetV1<StreamingResolver>,
    Arc<Observations>,
) {
    assert_ne!(maximum_read, 0);
    let observed = Arc::new(Observations::default());
    let resolver = StreamingResolver {
        bytes: bytes.into(),
        maximum_read,
        fault,
        observed: Arc::clone(&observed),
    };
    (
        KagemushaAuthenticatedArtifactSetV1::for_stream_tests(resolver, binding),
        observed,
    )
}

fn read_all(reader: &mut CapturedProvingKeyReaderV1<'_>) -> Vec<u8> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).expect("read original bytes");
    bytes
}

fn assert_empty(directory: &std::path::Path) {
    // This asserts pathname cleanup only. It does not observe detached descriptors or key bytes.
    assert_eq!(
        fs::read_dir(directory)
            .expect("read private directory")
            .count(),
        0
    );
}

#[cfg(unix)]
#[test]
fn capture_stream_roundtrips_every_proving_key_role_and_tail_geometry() {
    let directory = tempfile::tempdir().expect("private capture directory");
    let lengths = [1, WIDTH - 1, WIDTH, WIDTH + 1, 2 * WIDTH + 23];
    let mut cases = 0;
    for role in KagemushaArtifactRoleV1::ALL {
        if KagemushaArtifactDescriptorV1::for_role(role).kind != KagemushaArtifactKindV1::ProvingKey
        {
            continue;
        }
        for length in lengths {
            let bytes = pattern(length);
            let expected = binding(role, &bytes);
            let (set, observed) =
                fixture(bytes.clone(), expected, 37, SourceFault::InterruptedAt(0));
            let captured = set
                .capture_proving_key(role, directory.path())
                .expect("capture");
            assert_eq!(captured.binding(), expected);
            assert!(captured.matches_artifact_set(&set));
            assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
            assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
            assert_eq!(observed.bytes.load(Ordering::SeqCst), length);
            assert!(observed.largest_request.load(Ordering::SeqCst) <= WIDTH);
            let snapshot = captured.snapshot.as_ref().expect("sealed snapshot");
            assert_eq!(snapshot.slot_count_v1(), length.div_ceil(WIDTH) as u64);
            assert_eq!(snapshot.plaintext_len_v1(), WIDTH as u64);
            assert_eq!(
                snapshot.file_len_v1(),
                length.div_ceil(WIDTH) as u64 * (8192 + 16)
            );
            assert_ne!(snapshot.snapshot_digest_v1(), &[0; 32]);
            let (captured, ()) = captured
                .with_reader::<_, KagemushaArtifactErrorV1>(|reader| {
                    assert!(reader.cached.is_none());
                    assert_eq!(reader.read(&mut []).expect("empty read"), 0);
                    assert!(reader.cached.is_none());
                    assert_eq!(read_all(reader), bytes);
                    assert_eq!(reader.position, length as u64);
                    let cached = reader.cached.as_ref().expect("last cached chunk");
                    assert_eq!(cached.0, (length - 1) as u64 / 8192);
                    assert_eq!(cached.1.len_v1(), 8192);
                    let tail = (length - 1) % WIDTH + 1;
                    assert!(cached.1.as_slice_v1()[tail..].iter().all(|byte| *byte == 0));
                    let mut eof = [0xa5; 19];
                    assert_eq!(reader.read(&mut eof).expect("EOF"), 0);
                    assert_eq!(eof, [0xa5; 19]);
                    Ok(())
                })
                .expect("successful reader returns original owner");
            assert!(captured.matches_artifact_set(&set));
            let (_, second) = captured
                .with_reader::<_, KagemushaArtifactErrorV1>(|reader| {
                    assert_eq!(reader.position, 0);
                    assert!(reader.cached.is_none());
                    Ok(read_all(reader))
                })
                .expect("fresh callback restarts original stream");
            assert_eq!(second, bytes);
            assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
            assert_empty(directory.path());
            cases += 1;
        }
    }
    assert_eq!(cases, 24 * 5);
}

#[cfg(unix)]
#[test]
fn capture_reader_seek_matches_independent_cursor_and_bounds_one_cached_chunk() {
    let directory = tempfile::tempdir().expect("private capture directory");
    let bytes = pattern(3 * WIDTH + 29);
    let (set, _) = fixture(
        bytes.clone(),
        binding(ROLE, &bytes),
        WIDTH,
        SourceFault::None,
    );
    let captured = set
        .capture_proving_key(ROLE, directory.path())
        .expect("capture");
    let (_, ()) = captured
        .with_reader::<_, KagemushaArtifactErrorV1>(|reader| {
            let mut oracle = io::Cursor::new(bytes.as_slice());
            // A deterministic independent schedule repeatedly revisits and crosses all slots.
            let mut state = 0x517c_c1b7_u64;
            for iteration in 0..192 {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let position = state % (bytes.len() as u64 + 1);
                let command = match iteration % 3 {
                    0 => SeekFrom::Start(position),
                    1 => SeekFrom::Current(position as i64 - oracle.position() as i64),
                    _ => SeekFrom::End(position as i64 - bytes.len() as i64),
                };
                assert_eq!(
                    reader.seek(command).expect("bounded seek"),
                    oracle.seek(command).unwrap()
                );
                let count = (iteration * 173 % (WIDTH + 117)).min(bytes.len() - position as usize);
                let mut actual = vec![0x7d; count];
                let mut expected = vec![0; count];
                reader
                    .read_exact(&mut actual)
                    .expect("cross-slot exact read");
                oracle.read_exact(&mut expected).expect("oracle exact read");
                assert_eq!(actual, expected);
                assert_eq!(reader.position, oracle.position());
                if count != 0 {
                    let cached = reader.cached.as_ref().expect("one cached chunk");
                    assert_eq!(cached.0, (reader.position - 1) / 8192);
                    assert_eq!(cached.1.len_v1(), 8192);
                }
            }
            reader.seek(SeekFrom::Start(17)).unwrap();
            let mut first = [0; 3];
            reader.read_exact(&mut first).unwrap();
            let cached_pointer = reader.cached.as_ref().unwrap().1.as_slice_v1().as_ptr();
            reader.seek(SeekFrom::Start(91)).unwrap();
            reader.read_exact(&mut first).unwrap();
            assert_eq!(
                reader.cached.as_ref().unwrap().1.as_slice_v1().as_ptr(),
                cached_pointer
            );
            assert_eq!(first, bytes[91..94]);
            for invalid in [
                SeekFrom::Start(u64::MAX),
                SeekFrom::Start(bytes.len() as u64 + 1),
                SeekFrom::Current(i64::MIN),
                SeekFrom::Current(i64::MAX),
                SeekFrom::End(1),
                SeekFrom::End(i64::MIN),
                SeekFrom::End(i64::MAX),
            ] {
                let before = reader.position;
                assert_eq!(
                    reader.seek(invalid).unwrap_err().kind(),
                    io::ErrorKind::InvalidInput
                );
                assert_eq!(reader.position, before);
                assert!(reader.failure.is_none());
                assert!(reader.snapshot.is_some());
                assert_eq!(
                    reader.cached.as_ref().unwrap().1.as_slice_v1().as_ptr(),
                    cached_pointer
                );
            }
            assert_eq!(reader.seek(SeekFrom::End(0)).unwrap(), bytes.len() as u64);
            assert_eq!(reader.read(&mut [0; 33]).unwrap(), 0);
            reader.seek(SeekFrom::End(-1)).unwrap();
            let mut final_bytes = [0x9b; 7];
            assert_eq!(reader.read(&mut final_bytes).unwrap(), 1);
            assert_eq!(final_bytes[0], *bytes.last().unwrap());
            assert_eq!(final_bytes[1..], [0x9b; 6]);
            Ok(())
        })
        .expect("invalid seeks remain retryable");
    assert_empty(directory.path());
}

#[cfg(unix)]
#[test]
fn capture_rejects_role_geometry_and_original_stream_failures_before_escape() {
    let directory = tempfile::tempdir().expect("private capture directory");
    let bytes = pattern(2 * WIDTH + 13);
    let good = binding(ROLE, &bytes);
    let mut preflights = 0;
    for role in KagemushaArtifactRoleV1::ALL {
        if KagemushaArtifactDescriptorV1::for_role(role).kind == KagemushaArtifactKindV1::ProvingKey
        {
            continue;
        }
        let (set, observed) = fixture(bytes.clone(), good, WIDTH, SourceFault::None);
        assert!(
            matches!(set.capture_proving_key(role, directory.path()), Err(KagemushaArtifactErrorV1::InvalidBinding(rejected)) if rejected == role)
        );
        assert_eq!(observed.opens.load(Ordering::SeqCst), 0);
        preflights += 1;
    }
    assert_eq!(preflights, 26);
    for invalid in [
        KagemushaArtifactBindingV1 {
            byte_len: 0,
            ..good
        },
        KagemushaArtifactBindingV1 {
            byte_len: KagemushaArtifactDescriptorV1::for_role(ROLE).byte_limit + 1,
            ..good
        },
        KagemushaArtifactBindingV1 {
            sha256: [0; 32],
            ..good
        },
    ] {
        let (set, observed) = fixture(bytes.clone(), invalid, WIDTH, SourceFault::None);
        assert!(matches!(
            set.capture_proving_key(ROLE, directory.path()),
            Err(KagemushaArtifactErrorV1::InvalidBinding(ROLE))
        ));
        assert_eq!(observed.opens.load(Ordering::SeqCst), 0);
    }
    for case in 0..8 {
        let mut supplied = bytes.clone();
        let mut expected = good;
        let fault = match case {
            0 => {
                expected.sha256[0] ^= 1;
                SourceFault::None
            }
            1 => {
                supplied.pop();
                SourceFault::None
            }
            2 => {
                supplied.push(0x61);
                SourceFault::None
            }
            3 => SourceFault::ErrorAt(0),
            4 => SourceFault::ErrorAt(WIDTH + 7),
            5 => SourceFault::ErrorAt(bytes.len()),
            6 => SourceFault::Overreport,
            7 => {
                supplied[WIDTH + 5] ^= 1;
                SourceFault::None
            }
            _ => unreachable!(),
        };
        let (set, observed) = fixture(supplied, expected, 997, fault);
        let result = set.capture_proving_key(ROLE, directory.path());
        match case {
            0 | 7 => assert!(matches!(
                result,
                Err(KagemushaArtifactErrorV1::DigestMismatch(ROLE))
            )),
            2 => assert!(matches!(
                result,
                Err(KagemushaArtifactErrorV1::TrailingBytes(ROLE))
            )),
            _ => assert!(matches!(
                result,
                Err(KagemushaArtifactErrorV1::Read { role: ROLE, .. })
            )),
        }
        assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
        assert_empty(directory.path());
    }
    // Interrupted reads at the final EOF authentication gate must be retried as well.
    let (set, observed) = fixture(
        bytes.clone(),
        good,
        WIDTH,
        SourceFault::InterruptedAt(bytes.len()),
    );
    let captured = set
        .capture_proving_key(ROLE, directory.path())
        .expect("retry final EOF");
    assert_eq!(observed.bytes.load(Ordering::SeqCst), bytes.len());
    drop(captured);
    let (set, observed) = fixture(bytes, good, WIDTH, SourceFault::None);
    assert!(matches!(
        set.capture_proving_key(ROLE, &directory.path().join("missing")),
        Err(KagemushaArtifactErrorV1::Read { role: ROLE, .. })
    ));
    assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
    assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
    assert_eq!(observed.reads.load(Ordering::SeqCst), 0);
    assert_empty(directory.path());
}

#[cfg(unix)]
#[test]
fn capture_retains_all_original_identity_and_binds_spool_context() {
    let directory = tempfile::tempdir().expect("private capture directory");
    let bytes = pattern(WIDTH + 1);
    let expected = binding(ROLE, &bytes);
    let (set, _) = fixture(bytes.clone(), expected, WIDTH, SourceFault::None);
    let captured = set
        .capture_proving_key(ROLE, directory.path())
        .expect("capture");
    let original_context = captured.context;
    for case in 0..12 {
        let (mut other, _) = fixture(bytes.clone(), expected, WIDTH, SourceFault::None);
        match case {
            0 => other.recursion.release_id[0] ^= 1,
            1 => other.recursion.profile_digest[0] ^= 1,
            2 => other.recursion.artifact_manifest_digest[0] ^= 1,
            3 => other.native_profile_digest[0] ^= 1,
            4 => other.provider_policy_root[0] ^= 1,
            5 => other.suite_id[0] ^= 1,
            6 => other.vk_set_digest[0] ^= 1,
            7 => other.bindings[role_index(ROLE)].sha256[0] ^= 1,
            8 => other.bindings[role_index(ROLE)].byte_len -= 1,
            9 => other.recursion.eq_protocol_digest[0] ^= 1,
            10 => other.recursion.canonical_empty_effect_digest[0] ^= 1,
            11 => other.recursion.mint_finality.proving_key_ep.sha256[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(!captured.matches_artifact_set(&other));
        if case < 9 {
            let identity = OriginalArtifact::from_set(&other, ROLE).expect("valid identity shape");
            assert_ne!(
                identity
                    .context(identity.binding.byte_len.div_ceil(8192))
                    .unwrap(),
                original_context
            );
        }
    }
    // Complete retained recursion equality catches the remaining protocol inventory; the
    // explicit context fields bind its original release/manifest, not a re-authenticated fixture.
    let mut contexts = std::collections::BTreeSet::new();
    for role in KagemushaArtifactRoleV1::ALL {
        if KagemushaArtifactDescriptorV1::for_role(role).kind == KagemushaArtifactKindV1::ProvingKey
        {
            let original = OriginalArtifact::from_set(&set, role).unwrap();
            assert!(contexts.insert(original.context(2).unwrap()));
        }
    }
    assert_eq!(contexts.len(), 24);
    let (mut other, _) = fixture(bytes.clone(), expected, WIDTH, SourceFault::None);
    other.recursion.artifact_manifest_digest[0] ^= 1;
    let mut other_capture = other
        .capture_proving_key(ROLE, directory.path())
        .expect("other capture");
    let mut changed = captured;
    std::mem::swap(&mut changed.snapshot, &mut other_capture.snapshot);
    // The existing crypto API rejects an expected-context mismatch before decryption. The
    // capture wrapper deliberately consumes even that retryable lower-layer failure.
    let outcome = changed.with_reader::<_, KagemushaArtifactErrorV1>(|reader| {
        let mut output = [0xcc; 17];
        assert!(reader.read(&mut output).is_err());
        assert_eq!(output, [0xcc; 17]);
        assert!(reader.snapshot.is_none());
        assert!(reader.cached.is_none());
        assert!(reader.failure.is_some());
        assert!(reader.read(&mut []).is_err());
        assert!(reader.seek(SeekFrom::Start(0)).is_err());
        Ok(())
    });
    assert!(matches!(
        outcome,
        Err(KagemushaArtifactErrorV1::Read { role: ROLE, .. })
    ));
    drop(other_capture);
    assert_empty(directory.path());
}

#[derive(Debug)]
enum CallbackError {
    Artifact(KagemushaArtifactErrorV1),
    Deliberate,
}

impl From<KagemushaArtifactErrorV1> for CallbackError {
    fn from(error: KagemushaArtifactErrorV1) -> Self {
        Self::Artifact(error)
    }
}

struct ReturnedValue(Arc<AtomicUsize>);

impl Drop for ReturnedValue {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(unix)]
#[test]
fn capture_consumes_callback_errors_unwinds_and_swallowed_fatal_reads() {
    let directory = tempfile::tempdir().expect("private capture directory");
    let bytes = pattern(2 * WIDTH + 7);
    let (set, _) = fixture(
        bytes.clone(),
        binding(ROLE, &bytes),
        WIDTH,
        SourceFault::None,
    );
    let sentinel = set
        .capture_proving_key(ROLE, directory.path())
        .expect("unrelated capture");
    for case in 0..5 {
        let mut captured = set
            .capture_proving_key(ROLE, directory.path())
            .expect("capture");
        let dropped_value = Arc::new(AtomicUsize::new(0));
        if case == 4 {
            captured.snapshot.take();
            assert!(!captured.matches_artifact_set(&set));
        }
        let mut entered = false;
        let result = captured.with_reader::<_, CallbackError>(|reader| {
            entered = true;
            let mut prefix = [0; 13];
            reader.read_exact(&mut prefix).unwrap();
            assert_eq!(prefix, bytes[..13]);
            if case == 0 {
                return Err(CallbackError::Deliberate);
            }
            if case == 1 {
                reader.context[0] ^= 1;
                // A cache hit remains the already authenticated bytes. A distinct slot forces
                // an actual crypto-context check, then the complete capture must be consumed.
                reader.seek(SeekFrom::Start(8192)).unwrap();
                let mut output = [0x3d; 5];
                assert!(reader.read(&mut output).is_err());
                assert_eq!(output, [0x3d; 5]);
                assert!(reader.cached.is_none());
                assert!(reader.snapshot.is_none());
            } else if case == 2 {
                reader.snapshot.take();
                assert!(reader.read(&mut []).is_err());
            } else if case == 3 {
                reader.failure = Some(artifact_read_error(ROLE, "independent latched fault"));
                assert!(reader.read(&mut prefix).is_err());
            }
            Ok(ReturnedValue(Arc::clone(&dropped_value)))
        });
        match case {
            0 => assert!(matches!(result, Err(CallbackError::Deliberate))),
            _ => assert!(matches!(
                result,
                Err(CallbackError::Artifact(KagemushaArtifactErrorV1::Read {
                    role: ROLE,
                    ..
                }))
            )),
        }
        assert_eq!(entered, case != 4);
        assert_eq!(
            dropped_value.load(Ordering::SeqCst),
            usize::from((1..=3).contains(&case))
        );
        assert_empty(directory.path());
    }
    for offset in [0, WIDTH, 2 * WIDTH + 7] {
        let (faulty, observed) = fixture(
            bytes.clone(),
            binding(ROLE, &bytes),
            997,
            SourceFault::PanicAt(offset),
        );
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = faulty.capture_proving_key(ROLE, directory.path());
            }))
            .is_err()
        );
        assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
        assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
        assert_empty(directory.path());
    }
    for after_read in [false, true] {
        let captured = set
            .capture_proving_key(ROLE, directory.path())
            .expect("capture");
        let dropped_value = Arc::new(AtomicUsize::new(0));
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = captured.with_reader::<(), CallbackError>(|reader| {
                    let _probe = ReturnedValue(Arc::clone(&dropped_value));
                    if after_read {
                        reader.read_exact(&mut [0; 19]).unwrap();
                        assert!(reader.cached.is_some());
                    }
                    panic!("independent consumer unwind");
                });
            }))
            .is_err()
        );
        assert_eq!(dropped_value.load(Ordering::SeqCst), 1);
        assert_empty(directory.path());
    }
    let (_, actual) = sentinel
        .with_reader::<_, KagemushaArtifactErrorV1>(|reader| Ok(read_all(reader)))
        .expect("unrelated owner remains readable");
    assert_eq!(actual, bytes);
}

struct ReplacingResolver {
    resolver: KagemushaDirectoryArtifactResolverV1,
    path: PathBuf,
    displaced: PathBuf,
    replacement: Vec<u8>,
    opens: AtomicUsize,
}

impl KagemushaArtifactByteResolverV1 for ReplacingResolver {
    fn resolve_bytes(
        &self,
        _: KagemushaArtifactBindingV1,
    ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        panic!("whole-file resolution must not be used");
    }

    fn open_reader(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> Result<Box<dyn io::Read + Send>, KagemushaArtifactErrorV1> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        let reader = self.resolver.open_reader(binding)?;
        // Rename after the actual directory resolver opened and checked the original file,
        // before its 64 KiB BufReader has been consumed. A path reopen would see other bytes.
        fs::rename(&self.path, &self.displaced).expect("retain opened original inode");
        fs::write(&self.path, &self.replacement).expect("replace only source pathname");
        Ok(reader)
    }
}

#[cfg(unix)]
#[test]
fn capture_directory_source_uses_one_original_handle_and_never_reopens_path() {
    let source = tempfile::tempdir().expect("source directory");
    let destination = tempfile::tempdir().expect("capture directory");
    let bytes = pattern(9 * WIDTH + 19);
    let expected = binding(ROLE, &bytes);
    let path = source.path().join(lower_hex(expected.sha256));
    let replacement = vec![0xd3; bytes.len()];
    fs::write(&path, &bytes).expect("write original source");
    let resolver = ReplacingResolver {
        resolver: KagemushaDirectoryArtifactResolverV1::new(source.path())
            .expect("directory resolver"),
        path: path.clone(),
        displaced: source.path().join("original-displaced"),
        replacement: replacement.clone(),
        opens: AtomicUsize::new(0),
    };
    let set = KagemushaAuthenticatedArtifactSetV1::for_stream_tests(resolver, expected);
    let captured = set
        .capture_proving_key(ROLE, destination.path())
        .expect("original opened stream");
    assert_eq!(set.resolver.opens.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read(&path).unwrap(), replacement);
    fs::remove_file(path).expect("remove replacement pathname");
    fs::remove_file(&set.resolver.displaced).expect("remove original pathname");
    let (_, actual) = captured
        .with_reader::<_, KagemushaArtifactErrorV1>(|reader| Ok(read_all(reader)))
        .expect("spool remains independent of source paths");
    assert_eq!(actual, bytes);
    assert_eq!(set.resolver.opens.load(Ordering::SeqCst), 1);
    assert_empty(destination.path());
}

#[cfg(unix)]
#[test]
fn capture_one_byte_source_allows_partial_callbacks_without_recapturing() {
    let directory = tempfile::tempdir().expect("private capture directory");
    let bytes = pattern(WIDTH + 5);
    let expected = binding(ROLE, &bytes);
    let (set, observed) = fixture(
        bytes.clone(),
        expected,
        1,
        SourceFault::InterruptedAt(WIDTH + 1),
    );
    let captured = set
        .capture_proving_key(ROLE, directory.path())
        .expect("one-byte reads and a mid-tail interruption");
    assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
    assert_eq!(observed.drops.load(Ordering::SeqCst), 1);
    assert_eq!(observed.bytes.load(Ordering::SeqCst), bytes.len());
    let (captured, ()) = captured
        .with_reader::<_, KagemushaArtifactErrorV1>(|reader| {
            assert_eq!(reader.position, 0);
            assert!(reader.cached.is_none());
            Ok(())
        })
        .expect("no-op callback preserves the authenticated byte owner");
    let (captured, prefix) = captured
        .with_reader::<_, KagemushaArtifactErrorV1>(|reader| {
            let mut prefix = [0; 7];
            reader.read_exact(&mut prefix).expect("partial prefix");
            assert_eq!(reader.position, 7);
            Ok(prefix)
        })
        .expect("a range read need not decode the whole artifact");
    assert_eq!(prefix, bytes[..7]);
    assert!(captured.matches_artifact_set(&set));
    assert_eq!(captured.binding(), expected);
    let (_, actual) = captured
        .with_reader::<_, KagemushaArtifactErrorV1>(|reader| {
            assert_eq!(reader.position, 0);
            assert!(reader.cached.is_none());
            Ok(read_all(reader))
        })
        .expect("fresh callback reuses the captured original bytes");
    assert_eq!(actual, bytes);
    assert_eq!(observed.opens.load(Ordering::SeqCst), 1);
    assert_eq!(observed.bytes.load(Ordering::SeqCst), bytes.len());
    assert_empty(directory.path());
}

#[cfg(unix)]
#[test]
fn capture_directory_source_rejects_missing_nonregular_and_wrong_length_files() {
    let destination = tempfile::tempdir().expect("private capture directory");
    let bytes = pattern(WIDTH + 13);
    let expected = binding(ROLE, &bytes);
    for case in 0..4 {
        let source = tempfile::tempdir().expect("independent source directory");
        let resolver =
            KagemushaDirectoryArtifactResolverV1::new(source.path()).expect("directory resolver");
        let path = resolver.path_for_digest(expected.sha256);
        match case {
            0 => {}
            1 => fs::create_dir(&path).expect("nonregular content address"),
            2 => fs::write(&path, &bytes[..bytes.len() - 1]).expect("short source"),
            3 => {
                let mut extended = bytes.clone();
                extended.push(0x19);
                fs::write(&path, extended).expect("extended source");
            }
            _ => unreachable!(),
        }
        let set = KagemushaAuthenticatedArtifactSetV1::for_stream_tests(resolver, expected);
        let result = set.capture_proving_key(ROLE, destination.path());
        match case {
            0 | 1 => assert!(matches!(
                result,
                Err(KagemushaArtifactErrorV1::Read { role: ROLE, .. })
            )),
            2 | 3 => assert!(matches!(
                result,
                Err(KagemushaArtifactErrorV1::LengthMismatch {
                    role: ROLE,
                    expected: expected_length,
                    actual,
                }) if expected_length == bytes.len() as u64
                    && actual == (bytes.len() as u64 - 1 + 2 * u64::from(case == 3))
            )),
            _ => unreachable!(),
        }
        assert_empty(destination.path());
    }
}
