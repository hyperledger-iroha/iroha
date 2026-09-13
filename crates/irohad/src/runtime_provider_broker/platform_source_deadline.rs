/// Read a source transcript against one absolute operation deadline.
///
/// Nonblocking recv followed by bounded poll can drain a closed peer's buffered
/// trailer and EOF. Reconfiguring `SO_RCVTIMEO`/`SO_SNDTIMEO` instead fails with
/// `EINVAL` on macOS after peer close, even while authenticated bytes remain.
struct ProviderSourceDeadlineReader<'stream> {
    stream: &'stream UnixStream,
    deadline: std::time::Instant,
}
impl std::io::Read for ProviderSourceDeadlineReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        absolute_deadline::read_unix_before(self.stream, self.deadline, output)
    }
}

#[cfg(test)]
mod source_deadline_tests {
    use super::*;
    use std::io::{Read as _, Write as _};

    #[test]
    fn source_deadline_drains_buffered_bytes_after_peer_close() {
        let (stream, mut peer) = UnixStream::pair().expect("create source socket pair");
        peer.write_all(b"buffered source trailer")
            .expect("write buffered bytes");
        peer.shutdown(std::net::Shutdown::Write)
            .expect("half-close writer");
        drop(peer);
        let mut reader = ProviderSourceDeadlineReader {
            stream: &stream,
            deadline: std::time::Instant::now() + Duration::from_secs(1),
        };
        let mut observed = Vec::new();
        reader
            .read_to_end(&mut observed)
            .expect("drain closed peer through exact EOF");
        assert_eq!(observed, b"buffered source trailer");
    }

    #[test]
    fn source_deadline_does_not_restart_after_partial_reads() {
        let (stream, mut peer) = UnixStream::pair().expect("create source socket pair");
        peer.write_all(b"first").expect("write first partial frame");
        let deadline = std::time::Instant::now() + Duration::from_millis(30);
        let mut reader = ProviderSourceDeadlineReader {
            stream: &stream,
            deadline,
        };
        let mut first = [0; 5];
        reader
            .read_exact(&mut first)
            .expect("read first partial frame");
        assert_eq!(&first, b"first");
        let error = reader
            .read_exact(&mut [0; 1])
            .expect_err("one deadline bounds a partial frame");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(std::time::Instant::now() >= deadline);
        peer.write_all(b"late")
            .expect("buffer bytes after deadline");
        let error = reader
            .read(&mut [0; 4])
            .expect_err("expired deadline rejects buffered bytes");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }
}
