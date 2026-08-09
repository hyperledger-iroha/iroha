//! Bounded worker-to-consumer byte stream used by authenticated Musubi CAR fetches.

use std::{
    io::{self, Cursor, Read, Write},
    sync::mpsc,
    thread::{self, JoinHandle},
};

/// Maximum owned byte payload carried by one worker channel frame.
pub(super) const STREAM_FRAME_BYTES: usize = 32 * 1024;
/// Maximum number of byte frames retained ahead of the consumer.
pub(super) const STREAM_FRAME_COUNT: usize = 4;
/// Maximum concurrently owned frame bytes at the channel boundary.
///
/// In addition to the bounded queue, the consumer may own its current frame
/// while the producer owns the next frame whose send is blocked on a full
/// queue.
pub(super) const STREAM_MAX_OWNED_FRAME_BYTES: usize =
    STREAM_FRAME_BYTES * (STREAM_FRAME_COUNT + 2);

enum StreamMessageV1 {
    Data(Vec<u8>),
    Done,
    Failed(&'static str),
}

struct ChannelCarWriterV1<'sender> {
    sender: &'sender mpsc::SyncSender<StreamMessageV1>,
}

impl Write for ChannelCarWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for frame in bytes.chunks(STREAM_FRAME_BYTES) {
            self.sender
                .send(StreamMessageV1::Data(frame.to_vec()))
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "CAR reader closed"))?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ChannelCarReaderV1 {
    receiver: Option<mpsc::Receiver<StreamMessageV1>>,
    worker: Option<JoinHandle<()>>,
    current: Cursor<Vec<u8>>,
    expected_car_size: u64,
    received: u64,
    finished: bool,
}

impl ChannelCarReaderV1 {
    fn join_finished_worker(&mut self) -> io::Result<()> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        worker
            .join()
            .map_err(|_| io::Error::other("CAR stream worker panicked"))
    }

    fn close_and_join_worker(&mut self) -> io::Result<()> {
        drop(self.receiver.take());
        self.join_finished_worker()
    }

    fn fail_after_join(&mut self, error: io::Error) -> io::Result<usize> {
        self.close_and_join_worker()?;
        Err(error)
    }
}

impl Drop for ChannelCarReaderV1 {
    fn drop(&mut self) {
        // Closing the receiver releases a producer blocked on the bounded
        // channel. Production network reads have an explicit request timeout,
        // so joining here cannot leave an untracked worker after the reader is
        // dropped.
        let _ = self.close_and_join_worker();
    }
}

impl Read for ChannelCarReaderV1 {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let read = self.current.read(output)?;
            if read != 0 {
                let received = self
                    .received
                    .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
                    .ok_or_else(|| io::Error::other("CAR stream byte count overflow"));
                let received = match received {
                    Ok(received) => received,
                    Err(error) => return self.fail_after_join(error),
                };
                self.received = received;
                if received > self.expected_car_size {
                    return self.fail_after_join(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "CAR stream exceeded its commitment",
                    ));
                }
                return Ok(read);
            }
            if self.finished {
                return Ok(0);
            }
            let message = self
                .receiver
                .as_ref()
                .expect("unfinished CAR stream retains its receiver")
                .recv();
            match message {
                Ok(StreamMessageV1::Data(bytes)) if !bytes.is_empty() => {
                    self.current = Cursor::new(bytes);
                }
                Ok(StreamMessageV1::Data(_)) => {
                    return self.fail_after_join(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "CAR stream contained an empty frame",
                    ));
                }
                Ok(StreamMessageV1::Done) => {
                    self.join_finished_worker()?;
                    if self.received != self.expected_car_size {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "CAR stream ended before its committed size",
                        ));
                    }
                    self.finished = true;
                }
                Ok(StreamMessageV1::Failed(code)) => {
                    self.join_finished_worker()?;
                    return Err(io::Error::other(code));
                }
                Err(_) => {
                    self.join_finished_worker()?;
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "CAR stream worker stopped unexpectedly",
                    ));
                }
            }
        }
    }
}

/// Spawn one bounded CAR producer and return its exact-size consumer.
///
/// The producer may queue at most four 32 KiB frames ahead of the consumer;
/// total ownership also accounts for the consumer's current frame and a
/// producer frame blocked on a full queue. Terminal reads and reader drop join
/// the worker, so callers do not retain detached worker allocations.
pub(super) fn bounded_car_reader<F>(
    expected_car_size: u64,
    worker: F,
) -> io::Result<Box<dyn Read + Send + 'static>>
where
    F: FnOnce(&mut dyn Write) -> Result<(), &'static str> + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(STREAM_FRAME_COUNT);
    let worker = thread::Builder::new()
        .name("musubi-sorafs-car-v1".to_owned())
        .spawn(move || {
            let mut output = ChannelCarWriterV1 { sender: &sender };
            match worker(&mut output) {
                Ok(()) => {
                    let _ = sender.send(StreamMessageV1::Done);
                }
                Err(code) => {
                    let _ = sender.send(StreamMessageV1::Failed(code));
                }
            }
        })?;
    Ok(Box::new(ChannelCarReaderV1 {
        receiver: Some(receiver),
        worker: Some(worker),
        current: Cursor::new(Vec::new()),
        expected_car_size,
        received: 0,
        finished: false,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    #[test]
    fn owned_frame_reserve_includes_queue_producer_and_consumer() {
        assert_eq!(STREAM_FRAME_COUNT, 4);
        assert_eq!(STREAM_MAX_OWNED_FRAME_BYTES, STREAM_FRAME_BYTES * 6);
    }

    #[test]
    fn exact_stream_joins_worker_before_eof() {
        struct Completion(Arc<AtomicBool>);

        impl Drop for Completion {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let completed = Arc::new(AtomicBool::new(false));
        let completion = Arc::clone(&completed);
        let mut reader = bounded_car_reader(3, move |output| {
            let _completion = Completion(completion);
            output.write_all(&[1, 2, 3]).map_err(|_| "WRITE_FAILED")
        })
        .expect("spawn bounded stream");

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).expect("exact stream");
        assert_eq!(bytes, [1, 2, 3]);
        assert!(completed.load(Ordering::Acquire));
    }

    #[test]
    fn short_stream_fails_after_joining_worker() {
        let mut reader = bounded_car_reader(4, |output| {
            output.write_all(&[1, 2, 3]).map_err(|_| "WRITE_FAILED")
        })
        .expect("spawn bounded stream");

        let error = io::copy(&mut reader, &mut io::sink()).expect_err("short stream must fail");
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn oversized_stream_fails_at_the_committed_boundary() {
        let completed = Arc::new(AtomicBool::new(false));
        let completion = Arc::clone(&completed);
        let mut reader = bounded_car_reader(3, move |output| {
            struct Completion(Arc<AtomicBool>);
            impl Drop for Completion {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::Release);
                }
            }
            let _completion = Completion(completion);
            output.write_all(&[1, 2, 3, 4]).map_err(|_| "WRITE_FAILED")
        })
        .expect("spawn bounded stream");

        let error = io::copy(&mut reader, &mut io::sink()).expect_err("long stream must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(error.to_string(), "CAR stream exceeded its commitment");
        assert!(completed.load(Ordering::Acquire));
    }

    #[test]
    fn producer_failure_is_returned_after_joining_the_worker() {
        let completed = Arc::new(AtomicBool::new(false));
        let completion = Arc::clone(&completed);
        let mut reader = bounded_car_reader(1, move |_| {
            completion.store(true, Ordering::Release);
            Err("MUSUBI_TEST_STREAM_FAILED")
        })
        .expect("spawn bounded stream");

        let error = io::copy(&mut reader, &mut io::sink()).expect_err("producer must fail");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "MUSUBI_TEST_STREAM_FAILED");
        assert!(completed.load(Ordering::Acquire));
    }

    #[test]
    fn producer_panic_is_not_reported_as_clean_eof() {
        let mut reader = bounded_car_reader(1, |_| -> Result<(), &'static str> {
            panic!("injected producer panic")
        })
        .expect("spawn bounded stream");

        let error = io::copy(&mut reader, &mut io::sink()).expect_err("panic must fail");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "CAR stream worker panicked");
    }

    #[test]
    fn dropping_a_partial_reader_closes_and_joins_the_worker() {
        struct Completion(Arc<AtomicBool>);

        impl Drop for Completion {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }

        let completed = Arc::new(AtomicBool::new(false));
        let completion = Arc::clone(&completed);
        let reader = bounded_car_reader(u64::MAX, move |output| {
            let _completion = Completion(completion);
            loop {
                if output.write_all(&[0_u8; STREAM_FRAME_BYTES]).is_err() {
                    return Err("WRITE_FAILED");
                }
            }
        })
        .expect("spawn bounded stream");

        drop(reader);
        assert!(completed.load(Ordering::Acquire));
    }
}
