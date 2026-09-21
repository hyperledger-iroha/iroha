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
        read_unix_before(self.stream, self.deadline.expires_at(), buffer)
    }
}

/// Drain buffered bytes and exact EOF without changing a closed socket's options.
/// Every retry uses the original deadline, including interrupted readiness waits.
pub(super) fn read_unix_before(
    stream: &UnixStream,
    expires_at: Instant,
    buffer: &mut [u8],
) -> io::Result<usize> {
    let deadline = BrokerDeadlineV1 { expires_at };
    deadline.io_remaining()?;
    if buffer.is_empty() {
        return Ok(0);
    }
    socket_io_before(stream, deadline, rustix::event::PollFlags::IN, || {
        rustix::net::recv(stream, &mut *buffer, rustix::net::RecvFlags::DONTWAIT)
            .map(|(read, _)| read)
    })
}

impl Write for DeadlineUnixStreamV1<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.deadline.io_remaining()?;
        if buffer.is_empty() {
            return Ok(0);
        }
        // macOS has no MSG_NOSIGNAL. Fail closed if its socket-level equivalent
        // cannot be established; never rely on the process's signal disposition.
        #[cfg(target_os = "macos")]
        rustix::net::sockopt::set_socket_nosigpipe(&*self.stream, true)?;
        let flags = rustix::net::SendFlags::DONTWAIT;
        #[cfg(target_os = "linux")]
        let flags = flags | rustix::net::SendFlags::NOSIGNAL;
        socket_io_before(
            self.stream,
            self.deadline,
            rustix::event::PollFlags::OUT,
            || rustix::net::send(&*self.stream, buffer, flags),
        )
    }

    fn flush(&mut self) -> io::Result<()> {
        self.deadline.io_remaining()?;
        let result = self.stream.flush();
        self.deadline.io_remaining()?;
        result
    }
}

// Per-call nonblocking I/O never changes the descriptor's mode. In particular,
// do not reconfigure SO_RCVTIMEO after peer close: macOS can return EINVAL while
// the peer's final authenticated response remains buffered in the socket.
fn socket_io_before(
    stream: &UnixStream,
    deadline: BrokerDeadlineV1,
    readiness: rustix::event::PollFlags,
    mut operation: impl FnMut() -> rustix::io::Result<usize>,
) -> io::Result<usize> {
    loop {
        deadline.io_remaining()?;
        let result = operation();
        deadline.io_remaining()?;
        match result {
            Ok(count) => return Ok(count),
            Err(rustix::io::Errno::INTR) => continue,
            Err(rustix::io::Errno::AGAIN) => {}
            Err(error) => return Err(error.into()),
        }
        // macOS poll accepts at most i32::MAX milliseconds. Bounded slices
        // retain the same absolute deadline for longer source-stream limits.
        let timeout = rustix::event::Timespec::try_from(
            deadline
                .io_remaining()?
                .min(Duration::from_millis(i32::MAX as u64)),
        )
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "broker poll timeout"))?;
        let mut fds = [rustix::event::PollFd::new(stream, readiness)];
        let result = rustix::event::poll(&mut fds, Some(&timeout));
        deadline.io_remaining()?;
        match result {
            Ok(_) if fds[0].revents().contains(rustix::event::PollFlags::NVAL) => {
                return Err(rustix::io::Errno::BADF.into());
            }
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
        // Readiness, including HUP, is not authenticated EOF: recv must drain
        // the remaining bytes and report EOF itself before the same deadline.
    }
}

#[cfg(test)]
#[path = "absolute_deadline_tests.rs"]
mod tests;
