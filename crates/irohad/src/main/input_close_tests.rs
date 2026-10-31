//! Portable controls for the daemon's actual stdin watcher and supervisor signal.

use super::*;
use std::{
    io::{self, Write as _},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(10);

fn wait_for_shutdown(signal: &ShutdownSignal) {
    let deadline = Instant::now() + DEADLINE;
    while !signal.is_sent() {
        assert!(
            Instant::now() < deadline,
            "end-of-file must send the shutdown signal"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn drain_until_eof_consumes_the_whole_input() {
    let mut input = io::Cursor::new(vec![0x5A; 1_000]);
    drain_until_eof(&mut input);
    assert_eq!(input.position(), 1_000);
}

#[test]
fn closing_the_input_sends_the_shutdown_signal() {
    let (reader, mut writer) = io::pipe().expect("pipe");
    let signal = ShutdownSignal::new();
    spawn_input_close_shutdown(reader, signal.clone()).expect("watch input");
    writer.write_all(b"keepalive").expect("write to the pipe");
    std::thread::sleep(Duration::from_millis(50));
    assert!(!signal.is_sent(), "open input keeps the node running");
    drop(writer);
    wait_for_shutdown(&signal);
}

enum ReadStep {
    Data(&'static [u8]),
    Error(io::ErrorKind),
}

struct ControlledReader {
    steps: mpsc::Receiver<ReadStep>,
    entered: mpsc::Sender<()>,
}

impl io::Read for ControlledReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let _ = self.entered.send(());
        match self.steps.recv() {
            Ok(ReadStep::Data(bytes)) => {
                assert!(bytes.len() <= buffer.len());
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            Ok(ReadStep::Error(kind)) => Err(kind.into()),
            Err(_) => Ok(0),
        }
    }
}

fn controlled_input() -> (ControlledReader, mpsc::Sender<ReadStep>, mpsc::Receiver<()>) {
    let (steps_tx, steps) = mpsc::channel();
    let (entered, entered_rx) = mpsc::channel();
    (ControlledReader { steps, entered }, steps_tx, entered_rx)
}

#[test]
fn data_and_interruption_keep_the_original_reader_alive_until_eof() {
    let (reader, steps, entered) = controlled_input();
    let signal = ShutdownSignal::new();
    spawn_input_close_shutdown(reader, signal.clone()).expect("watch controlled input");
    entered.recv_timeout(DEADLINE).expect("first blocking read");
    assert!(
        !signal.is_sent(),
        "an open blocked input keeps the node running"
    );
    for step in [
        ReadStep::Error(io::ErrorKind::Interrupted),
        ReadStep::Data(b"keepalive"),
        ReadStep::Error(io::ErrorKind::Interrupted),
        ReadStep::Data(b"more data"),
    ] {
        steps.send(step).expect("release one read");
        entered
            .recv_timeout(DEADLINE)
            .expect("watcher must resume its original blocking read");
        assert!(
            !signal.is_sent(),
            "neither data nor an interrupted read requests shutdown"
        );
    }
    drop(steps);
    wait_for_shutdown(&signal);
}

#[test]
fn an_input_error_sends_the_actual_shutdown_signal() {
    let (reader, steps, entered) = controlled_input();
    let signal = ShutdownSignal::new();
    spawn_input_close_shutdown(reader, signal.clone()).expect("watch failing input");
    entered.recv_timeout(DEADLINE).expect("first blocking read");
    assert!(!signal.is_sent());
    steps
        .send(ReadStep::Error(io::ErrorKind::Other))
        .expect("deliver the real Read error");
    wait_for_shutdown(&signal);
    assert!(
        steps.send(ReadStep::Data(b"late")).is_err(),
        "the failed original reader must be released"
    );
}

struct CountedReader<R> {
    inner: R,
    bytes: Arc<AtomicUsize>,
}

impl<R: io::Read> io::Read for CountedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.bytes.fetch_add(count, Ordering::Relaxed);
        Ok(count)
    }
}

const CHILD_FIXTURE: &str = "IROHAD_INPUT_CLOSE_TEST_CHILD";

#[test]
fn stdin_shutdown_child_fixture() {
    let Some(mode) = std::env::var_os(CHILD_FIXTURE) else {
        return;
    };
    assert_eq!(mode, "stdin");
    let signal = ShutdownSignal::new();
    let bytes = Arc::new(AtomicUsize::new(0));
    spawn_input_close_shutdown(
        CountedReader {
            inner: io::stdin(),
            bytes: Arc::clone(&bytes),
        },
        signal.clone(),
    )
    .expect("production watcher on the actual child stdin pipe");
    let mut output = io::stdout().lock();
    writeln!(output, "\ninput-close-ready").expect("ready receipt");
    output.flush().expect("flush ready receipt");
    let deadline = Instant::now() + DEADLINE;
    while bytes.load(Ordering::Relaxed) != 1_000 {
        assert!(
            Instant::now() < deadline,
            "original stdin must deliver all data"
        );
        assert!(
            !signal.is_sent(),
            "the input must remain open until the parent's close"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!signal.is_sent(), "data is not an EOF shutdown request");
    writeln!(output, "input-close-data=1000").expect("data receipt");
    output.flush().expect("flush data receipt");
    wait_for_shutdown(&signal);
    assert_eq!(bytes.load(Ordering::Relaxed), 1_000);
    writeln!(output, "input-close-shutdown").expect("shutdown receipt");
    output.flush().expect("flush shutdown receipt");
}

fn read_receipt(reader: &mut impl io::BufRead, expected: &str) {
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).expect("child receipt"), 0);
        if line.trim_end() == expected {
            break;
        }
    }
}

struct ChildFixture(std::process::Child);

impl Drop for ChildFixture {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            // Only failed assertions reach cleanup with this fixture still running.
            // This owns the actual child; cleanup is never a successful EOF receipt.
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

#[test]
fn actual_child_stdin_data_and_original_pipe_close_drive_shutdown() {
    use std::process::{Command, Stdio};
    let (_, module) = module_path!().split_once("::").expect("test module path");
    let child = Command::new(std::env::current_exe().expect("test executable"))
        .args([
            "--exact",
            &format!("{module}::stdin_shutdown_child_fixture"),
            "--nocapture",
        ])
        .env(CHILD_FIXTURE, "stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn actual native stdin fixture");
    let mut fixture = ChildFixture(child);
    let child = &mut fixture.0;
    let mut input = child.stdin.take().expect("retain original child input");
    let mut output = io::BufReader::new(child.stdout.take().expect("child output"));
    read_receipt(&mut output, "input-close-ready");
    assert!(child.try_wait().expect("live child").is_none());
    input
        .write_all(&[0x5A; 1_000])
        .expect("write original pipe");
    input.flush().expect("flush original pipe");
    read_receipt(&mut output, "input-close-data=1000");
    assert!(
        child
            .try_wait()
            .expect("data keeps the child live")
            .is_none()
    );
    drop(input);
    read_receipt(&mut output, "input-close-shutdown");
    let status = child.wait().expect("reap original child");
    assert!(status.success(), "native watcher child failed: {status:?}");
}
