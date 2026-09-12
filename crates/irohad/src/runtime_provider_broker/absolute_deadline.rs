//! One monotonic client exchange deadline across lock admission and partial socket I/O.

use super::BrokerError;
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    sync::{Mutex, MutexGuard, TryLockError},
    time::{Duration, Instant},
};

/// A copied deadline retains its original endpoint; it never renews an exchange.
#[derive(Clone, Copy)]
pub(super) struct BrokerDeadlineV1 {
    expires_at: Instant,
}

impl BrokerDeadlineV1 {
    pub(super) fn new(timeout: Duration) -> Result<Self, BrokerError> {
        if timeout.is_zero() {
            return Err(BrokerError::Unavailable);
        }
        Ok(Self {
            expires_at: Instant::now()
                .checked_add(timeout)
                .ok_or(BrokerError::Unavailable)?,
        })
    }

    pub(super) const fn expires_at(self) -> Instant {
        self.expires_at
    }

    pub(super) fn remaining(self) -> Result<Duration, BrokerError> {
        self.expires_at
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(BrokerError::Unavailable)
    }

    /// Only blocking workers use this admission loop. Waiting does not retire a request ID.
    pub(super) fn lock<T>(self, mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, BrokerError> {
        loop {
            self.remaining()?;
            match mutex.try_lock() {
                Ok(guard) => {
                    self.remaining()?;
                    return Ok(guard);
                }
                Err(TryLockError::Poisoned(_)) => return Err(BrokerError::Unavailable),
                Err(TryLockError::WouldBlock) => {
                    std::thread::park_timeout(self.remaining()?.min(Duration::from_millis(1)));
                }
            }
        }
    }

    fn io_remaining(self) -> io::Result<Duration> {
        self.remaining()
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "broker exchange deadline"))
    }
}

/// Existing bounded frame codecs call this wrapper for every partial read/write.
pub(super) struct DeadlineUnixStreamV1<'stream> {
    stream: &'stream mut UnixStream,
    deadline: BrokerDeadlineV1,
}

impl<'stream> DeadlineUnixStreamV1<'stream> {
    pub(super) fn new(stream: &'stream mut UnixStream, deadline: BrokerDeadlineV1) -> Self {
        Self { stream, deadline }
    }
}

impl Read for DeadlineUnixStreamV1<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream
            .set_read_timeout(Some(self.deadline.io_remaining()?))?;
        let result = self.stream.read(buffer);
        self.deadline.io_remaining()?;
        result
    }
}

impl Write for DeadlineUnixStreamV1<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(Some(self.deadline.io_remaining()?))?;
        let result = self.stream.write(buffer);
        self.deadline.io_remaining()?;
        result
    }

    fn flush(&mut self) -> io::Result<()> {
        self.deadline.io_remaining()?;
        let result = self.stream.flush();
        self.deadline.io_remaining()?;
        result
    }
}

#[cfg(test)]
#[path = "absolute_deadline_tests.rs"]
mod tests;
