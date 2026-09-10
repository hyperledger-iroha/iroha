//! Actual transport exchanges over retained local descriptors, without any child process.

use super::*;
use std::{
    io,
    net::Shutdown,
    os::{
        fd::BorrowedFd,
        unix::{fs::OpenOptionsExt, net::UnixStream},
    },
    sync::atomic::AtomicUsize,
};

const TEST_TIMEOUT: Duration = Duration::from_secs(2);

struct Peer {
    input: UnixStream,
    output: UnixStream,
}
impl Peer {
    fn request(&mut self, expected: Request) {
        let mut bytes = Vec::new();
        loop {
            let mut byte = [0_u8];
            self.input
                .read_exact(&mut byte)
                .expect("bounded request read");
            bytes.push(byte[0]);
            assert!(bytes.len() <= MAX_IPC_BYTES);
            if byte[0] == b'\n' {
                break;
            }
        }
        assert_eq!(bytes, expected.line().unwrap());
    }
    fn response(&mut self, bytes: &[u8]) {
        self.output
            .write_all(bytes)
            .expect("bounded response write");
    }
}
fn fixture() -> (tempfile::TempDir, Pipe<UnixStream, UnixStream>, Peer) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().join("captures");
    let captures = Captures::create(&path).unwrap();
    let (stdin, input) = UnixStream::pair().unwrap();
    let (stdout, output) = UnixStream::pair().unwrap();
    for stream in [&stdin, &stdout] {
        stream.set_nonblocking(true).unwrap();
    }
    for stream in [&input, &output] {
        stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(TEST_TIMEOUT)).unwrap();
    }
    let control = Arc::new(Control {
        origin: WallInstant::now(),
        lifetime_ns: 10_000_000_000,
        request_deadline_ns: AtomicU64::new(10_000_000_000),
        stop: AtomicBool::new(false),
        exit: AtomicU8::new(0),
    });
    (
        dir,
        Pipe {
            stdin: Some(stdin),
            stdout,
            control,
            captures,
        },
        Peer { input, output },
    )
}
fn request(kind: Kind, sequence: u64) -> Request {
    Request {
        kind,
        sequence,
        timeout_ms: 2000,
    }
}
fn deadline(pipe: &Pipe<impl Write + AsFd, impl Read + AsFd>) -> u64 {
    pipe.control.now() + u64::try_from(TEST_TIMEOUT.as_nanos()).unwrap()
}
fn response(request: Request, outcome: &str, manifest: Option<&Manifest>) -> Vec<u8> {
    let mut bytes = json::to_vec(&norito::json!({"schema": RESPONSE_SCHEMA, "kind": (request.kind.text()),
        "sequence": (request.sequence), "outcome": outcome, "manifest": (manifest.map(Manifest::value))})).unwrap();
    bytes.push(b'\n');
    bytes
}
fn publish(captures: &Captures, request: Request) -> Manifest {
    let bytes = json::to_vec(
        &norito::json!({"schema": CAPTURE_SCHEMA,"kind": (request.kind.text()),
        "sequence": (request.sequence),"available": true}),
    )
    .unwrap();
    let manifest = Manifest {
        name: request.kind.manifest_name(request.sequence),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        bytes: bytes.len() as u64,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(captures.path.join(&manifest.name))
        .unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    captures.directory.sync_all().unwrap();
    manifest
}

struct Limited<T> {
    inner: T,
    turn: bool,
    calls: Arc<AtomicUsize>,
}
impl<T: AsFd> AsFd for Limited<T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}
impl<T: Write> Write for Limited<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.turn = !self.turn;
        if self.turn {
            Err(io::ErrorKind::WouldBlock.into())
        } else {
            self.inner.write(&bytes[..bytes.len().min(3)])
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl<T: Read> Read for Limited<T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.turn = !self.turn;
        if self.turn {
            Err(io::ErrorKind::WouldBlock.into())
        } else {
            let capacity = bytes.len().min(3);
            self.inner.read(&mut bytes[..capacity])
        }
    }
}

#[test]
fn actual_exchange_reassembles_partial_nonblocking_io_and_authenticates_capture() {
    let (_dir, pipe, mut peer) = fixture();
    let expected = request(Kind::Sample, 1);
    let manifest = publish(&pipe.captures, expected);
    let bytes = response(expected, "complete", Some(&manifest));
    let writer_calls = Arc::new(AtomicUsize::new(0));
    let reader_calls = Arc::new(AtomicUsize::new(0));
    let mut pipe = Pipe {
        stdin: Some(Limited {
            inner: pipe.stdin.unwrap(),
            turn: false,
            calls: writer_calls.clone(),
        }),
        stdout: Limited {
            inner: pipe.stdout,
            turn: false,
            calls: reader_calls.clone(),
        },
        control: pipe.control,
        captures: pipe.captures,
    };
    let serve = std::thread::spawn(move || {
        peer.request(expected);
        peer.response(&bytes);
    });
    let result = pipe.exchange(expected, deadline(&pipe)).unwrap();
    serve.join().unwrap();
    assert_eq!(result.outcome, Outcome::Complete);
    assert_eq!(result.manifest, Some(manifest));
    assert!(writer_calls.load(Ordering::SeqCst) > 10);
    assert!(reader_calls.load(Ordering::SeqCst) > 10);
    assert!(pipe.stdin.is_some());
}

#[test]
fn actual_exchange_rejects_unsolicited_output_before_writing_any_request() {
    let (_dir, mut pipe, mut peer) = fixture();
    peer.response(b"{}\n");
    assert!(
        pipe.exchange(request(Kind::Sample, 1), deadline(&pipe))
            .is_err()
    );
    peer.input.set_nonblocking(true).unwrap();
    assert_eq!(
        peer.input.read(&mut [0_u8]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn actual_exchange_rejects_early_response_eof_and_closed_request_pipe() {
    for close_input in [false, true] {
        let (_dir, mut pipe, mut peer) = fixture();
        let expected = request(Kind::Sample, 1);
        if close_input {
            peer.input.shutdown(Shutdown::Read).unwrap();
            assert!(pipe.exchange(expected, deadline(&pipe)).is_err());
        } else {
            let serve = std::thread::spawn(move || {
                peer.request(expected);
                peer.output.shutdown(Shutdown::Write).unwrap();
            });
            assert!(pipe.exchange(expected, deadline(&pipe)).is_err());
            serve.join().unwrap();
        }
    }
}

#[test]
fn actual_exchange_rejects_oversize_extra_lines_and_wrong_sequence() {
    for mode in 0..3 {
        let (_dir, mut pipe, mut peer) = fixture();
        let expected = request(Kind::Sample, 1);
        let manifest = publish(&pipe.captures, expected);
        let bytes = match mode {
            0 => vec![b' '; MAX_IPC_BYTES + 1],
            1 => [
                response(expected, "complete", Some(&manifest)),
                b"{}\n".to_vec(),
            ]
            .concat(),
            _ => response(request(Kind::Sample, 2), "complete", Some(&manifest)),
        };
        let serve = std::thread::spawn(move || {
            peer.request(expected);
            peer.response(&bytes);
            peer
        });
        let first = pipe.exchange(expected, deadline(&pipe));
        let mut peer = serve.join().unwrap();
        if mode == 1 && first.is_ok() {
            // Stream chunk boundaries are deliberately not assumed. Extra data
            // must be rejected before the next request if it arrived after LF.
            assert!(
                pipe.exchange(request(Kind::Sample, 2), deadline(&pipe))
                    .is_err()
            );
            peer.input.set_nonblocking(true).unwrap();
            assert_eq!(
                peer.input.read(&mut [0_u8]).unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
        } else {
            assert!(first.is_err());
        }
    }
}

#[test]
fn actual_exchange_cannot_reuse_delayed_extra_output_as_a_second_response() {
    let (_dir, mut pipe, mut peer) = fixture();
    let first = request(Kind::Sample, 1);
    let manifest = publish(&pipe.captures, first);
    let bytes = response(first, "complete", Some(&manifest));
    let (send_extra, receive_extra) = mpsc::sync_channel(1);
    let (extra_sent, sent) = mpsc::sync_channel(1);
    let serve = std::thread::spawn(move || {
        peer.request(first);
        peer.response(&bytes);
        receive_extra.recv_timeout(TEST_TIMEOUT).unwrap();
        peer.response(b"{}\n");
        extra_sent.send(()).unwrap();
        peer
    });
    assert_eq!(
        pipe.exchange(first, deadline(&pipe)).unwrap().outcome,
        Outcome::Complete
    );
    send_extra.send(()).unwrap();
    sent.recv_timeout(TEST_TIMEOUT).unwrap();
    assert!(
        pipe.exchange(request(Kind::Sample, 2), deadline(&pipe))
            .is_err()
    );
    let mut peer = serve.join().unwrap();
    peer.input.set_nonblocking(true).unwrap();
    assert_eq!(
        peer.input.read(&mut [0_u8]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn actual_exchange_obeys_response_deadline_without_waiting_for_peer_eof() {
    let (_dir, mut pipe, mut peer) = fixture();
    let expected = request(Kind::Sample, 1);
    let (finished, hold) = mpsc::sync_channel(1);
    let serve = std::thread::spawn(move || {
        peer.request(expected);
        hold.recv_timeout(TEST_TIMEOUT).unwrap();
    });
    let before = WallInstant::now();
    let deadline = pipe.control.now() + 250_000_000;
    assert!(pipe.exchange(expected, deadline).is_err());
    assert!(pipe.control.now() >= deadline);
    assert!(before.elapsed() < TEST_TIMEOUT);
    finished.send(()).unwrap();
    serve.join().unwrap();
}

#[test]
fn actual_exchange_checks_lifetime_and_cancellation_before_io_and_during_response() {
    let (_dir, mut pipe, mut peer) = fixture();
    let expected = request(Kind::Sample, 1);
    let control = pipe.control.clone();
    let (finished, hold) = mpsc::sync_channel(1);
    let serve = std::thread::spawn(move || {
        peer.request(expected);
        control.stop.store(true, Ordering::Release);
        // Retain both sockets until exchange returns: EOF cannot mask the
        // actual cancellation check being exercised by this case.
        hold.recv_timeout(TEST_TIMEOUT).unwrap();
    });
    assert!(pipe.exchange(expected, deadline(&pipe)).is_err());
    finished.send(()).unwrap();
    serve.join().unwrap();
    assert!(pipe.control.stop.load(Ordering::Acquire));
    for cancelled in [false, true] {
        let (_dir, mut pipe, mut peer) = fixture();
        if cancelled {
            pipe.control.stop.store(true, Ordering::Release);
        } else {
            Arc::get_mut(&mut pipe.control).unwrap().lifetime_ns = 0;
        }
        assert!(pipe.exchange(expected, deadline(&pipe)).is_err());
        peer.input.set_nonblocking(true).unwrap();
        assert_eq!(
            peer.input.read(&mut [0_u8]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}

#[test]
fn actual_finish_requires_both_clean_exit_and_eof_without_trailing_output() {
    // Exact clean exit+EOF succeeds. EOF alone, nonzero exit, clean exit without
    // EOF, and data following finish all fail the same production exchange.
    for mode in 0..5 {
        let (_dir, mut pipe, mut peer) = fixture();
        let expected = request(Kind::Finish, 2);
        let control = pipe.control.clone();
        let bytes = response(expected, "complete", None);
        let (finished, hold) = mpsc::sync_channel(1);
        let serve = std::thread::spawn(move || {
            peer.request(expected);
            peer.response(&bytes);
            // The actual finish owner closes its input after parsing the response.
            assert_eq!(peer.input.read(&mut [0_u8]).unwrap(), 0);
            match mode {
                0 => {
                    control.exit.store(1, Ordering::Release);
                    peer.output.shutdown(Shutdown::Write).unwrap();
                }
                1 => {
                    peer.output.shutdown(Shutdown::Write).unwrap();
                }
                2 => {
                    control.exit.store(2, Ordering::Release);
                    peer.output.shutdown(Shutdown::Write).unwrap();
                }
                3 => {
                    control.exit.store(1, Ordering::Release);
                }
                _ => {
                    peer.response(b"!");
                    control.exit.store(1, Ordering::Release);
                    peer.output.shutdown(Shutdown::Write).unwrap();
                }
            }
            hold.recv_timeout(TEST_TIMEOUT).unwrap();
        });
        let end = pipe.control.now()
            + if mode == 0 {
                1_000_000_000
            } else {
                250_000_000
            };
        let result = pipe.exchange(expected, end);
        assert_eq!(result.is_ok(), mode == 0);
        assert!(pipe.stdin.is_none());
        finished.send(()).unwrap();
        serve.join().unwrap();
    }
}
