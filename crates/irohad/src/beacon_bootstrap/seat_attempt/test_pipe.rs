//! Portable FIFO descriptors for the original-input custody tests.

use std::os::fd::OwnedFd;

/// Construct both owned ends before exposing them to a test reader or writer.
pub(super) fn pipe(nonblocking: bool) -> (OwnedFd, OwnedFd) {
    let (read, write) = rustix::pipe::pipe().expect("test FIFO");
    for descriptor in [&read, &write] {
        rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::CLOEXEC)
            .expect("test FIFO close-on-exec");
        if nonblocking {
            let flags = rustix::fs::fcntl_getfl(descriptor).expect("test FIFO status flags");
            rustix::fs::fcntl_setfl(descriptor, flags | rustix::fs::OFlags::NONBLOCK)
                .expect("test FIFO nonblocking mode");
        }
    }
    (read, write)
}
