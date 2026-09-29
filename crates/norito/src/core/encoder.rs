//! Serialization destinations used by the Norito core codec.
use super::{ByteSink, Error, encode_writers::LengthCountingWriter};
use std::io::{self, Write};
/// Non-generic destination used by [`super::NoritoSerialize`] implementations.
///
/// Erasing the concrete [`Write`] type at the outer serialization boundary prevents every model
/// type from being monomorphized once per destination. A dedicated buffer path keeps the common
/// temporary-field and bare-payload encoders allocation-free beyond the buffer they already own.
pub struct Encoder<'a> {
    sink: EncoderSink<'a>,
    written: usize,
    limit: Option<usize>,
    rejected_write: bool,
}
enum EncoderSink<'a> {
    Buffer(&'a mut Vec<u8>),
    ByteSink(&'a mut ByteSink),
    Writer(&'a mut dyn Write),
    Counting(&'a mut LengthCountingWriter),
}
// The only scoped state is the active bound. Actual successful writes and an
// observed overrun survive errors and unwind, including errors a child ignores.
struct ExactLengthScope<'scope, 'sink> {
    encoder: &'scope mut Encoder<'sink>,
    previous_limit: Option<usize>,
}
impl Drop for ExactLengthScope<'_, '_> {
    fn drop(&mut self) {
        self.encoder.limit = self.previous_limit;
    }
}
impl<'a> Encoder<'a> {
    fn with_sink(sink: EncoderSink<'a>) -> Self {
        Self {
            sink,
            written: 0,
            limit: None,
            rejected_write: false,
        }
    }
    /// Create an encoder over an arbitrary byte writer.
    pub fn new(writer: &'a mut dyn Write) -> Self {
        Self::with_sink(EncoderSink::Writer(writer))
    }
    /// Create an encoder that appends directly to `buffer`.
    ///
    /// Use this when the caller needs owned payload bytes. Length-prefixed fields
    /// stream through their destination without staging in a separate buffer.
    /// The sink representation remains an implementation detail.
    #[doc(hidden)]
    pub fn for_buffer(buffer: &'a mut Vec<u8>) -> Self {
        Self::with_sink(EncoderSink::Buffer(buffer))
    }
    pub(super) fn for_byte_sink(sink: &'a mut ByteSink) -> Self {
        Self::with_sink(EncoderSink::ByteSink(sink))
    }
    pub(super) fn for_counting(counter: &'a mut LengthCountingWriter) -> Self {
        Self::with_sink(EncoderSink::Counting(counter))
    }

    pub(super) fn is_counting(&self) -> bool {
        matches!(self.sink, EncoderSink::Counting(_))
    }

    #[cold]
    fn reject_overrun(&mut self) -> io::Error {
        self.rejected_write = true;
        io::ErrorKind::InvalidData.into()
    }

    // Refuse the whole requested slice before forwarding any of it. This
    // preserves ExactLengthWriter's overrun boundary even for partial writers.
    fn admitted_end(&mut self, length: usize) -> io::Result<usize> {
        if self.rejected_write {
            return Err(self.reject_overrun());
        }
        let Some(end) = self.written.checked_add(length) else {
            if let EncoderSink::Counting(counter) = &mut self.sink {
                // The counter's total includes every successful encoder write
                // and possibly earlier encoders. Its matching failed addition
                // must remain sticky for LengthCountingWriter::finish, even if
                // a hostile serializer discards this I/O error.
                let rejected = counter.add(length);
                debug_assert!(rejected.is_err());
            }
            return Err(self.reject_overrun());
        };
        if self.limit.is_some_and(|limit| end > limit) {
            return Err(self.reject_overrun());
        }
        Ok(end)
    }

    /// Serialize a measured child through this encoder's original destination.
    ///
    /// The active bound is relative to the outermost child, and is the smaller
    /// of this child's end and its enclosing bound. Restoring it requires no writer wrapper, allocation, or
    /// per-byte traversal of ancestor scopes. Only successful writes advance
    /// the shared offset; a child overrun remains sticky after scope exit.
    pub(super) fn with_exact_length(
        &mut self,
        expected_len: usize,
        serialize: impl FnOnce(&mut Self) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if self.rejected_write {
            return Err(Error::LengthMismatch);
        }
        let previous_limit = self.limit;
        if previous_limit.is_none() {
            // Bytes outside every exact scope need no duplicate accounting.
            // Each outermost child starts a fresh relative coordinate system.
            self.written = 0;
        }
        let Some(expected_end) = self.written.checked_add(expected_len) else {
            self.rejected_write = true;
            return Err(Error::LengthMismatch);
        };
        self.limit = Some(previous_limit.map_or(expected_end, |limit| limit.min(expected_end)));
        let scope = ExactLengthScope {
            encoder: self,
            previous_limit,
        };
        let result = serialize(scope.encoder);
        if scope.encoder.rejected_write {
            return Err(Error::LengthMismatch);
        }
        result?;
        (scope.encoder.written == expected_end)
            .then_some(())
            .ok_or(Error::LengthMismatch)
    }

    // Only codec helpers which have themselves measured this child may use this.
    // Arbitrary writers, including checksum/comparison writers over io::sink(),
    // must still receive every actual byte. Counting is a property of this
    // destination, never a process/thread mode inherited by unrelated encoders.
    #[inline]
    pub(super) fn count_measured_bytes(&mut self, length: usize) -> Result<bool, Error> {
        if !self.is_counting() {
            return Ok(false);
        }
        if self.rejected_write {
            return Err(self.reject_overrun().into());
        }
        if self.limit.is_none() {
            if let EncoderSink::Counting(counter) = &mut self.sink {
                counter.add(length)?;
            }
            return Ok(true);
        }
        let end = self.admitted_end(length)?;
        if let EncoderSink::Counting(counter) = &mut self.sink {
            counter.add(length)?;
            self.written = end;
        }
        Ok(true)
    }
    /// Write an entire byte slice to the serialization destination.
    ///
    /// This inherent operation keeps generated implementations independent of
    /// whether [`Write`] happens to be imported at the derive site.
    #[inline]
    pub fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.rejected_write {
            return Err(self.reject_overrun());
        }
        if self.limit.is_some() {
            return self.write_all_bounded(bytes);
        }
        // Keep the unbounded entry small enough to inline direct destinations.
        // Only active exact scopes need successful-prefix accounting.
        match &mut self.sink {
            EncoderSink::Buffer(buffer) => {
                buffer.extend_from_slice(bytes);
                Ok(())
            }
            EncoderSink::ByteSink(sink) => {
                sink.write_bytes(bytes);
                Ok(())
            }
            EncoderSink::Writer(writer) => writer.write_all(bytes),
            EncoderSink::Counting(counter) => counter.write_all(bytes),
        }
    }
    fn write_all_bounded(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        let end = self.admitted_end(bytes.len())?;
        match &mut self.sink {
            EncoderSink::Buffer(buffer) => {
                buffer.extend_from_slice(bytes);
                self.written = end;
            }
            EncoderSink::ByteSink(sink) => {
                sink.write_bytes(bytes);
                self.written = end;
            }
            EncoderSink::Counting(counter) => {
                counter.write_all(bytes)?;
                self.written = end;
            }
            EncoderSink::Writer(_) => {
                // Delegating write_all loses the successful prefix length if
                // the destination writes partially and then returns an error.
                // Account for each actual write, including Interrupted retry.
                while !bytes.is_empty() {
                    match self.write_bounded(bytes) {
                        Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                        Ok(written) => bytes = &bytes[written..],
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                        Err(error) => return Err(error),
                    }
                }
            }
        }
        Ok(())
    }
    fn write_bounded(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.admitted_end(bytes.len())?;
        let written = match &mut self.sink {
            EncoderSink::Buffer(buffer) => {
                buffer.extend_from_slice(bytes);
                bytes.len()
            }
            EncoderSink::ByteSink(sink) => {
                sink.write_bytes(bytes);
                bytes.len()
            }
            EncoderSink::Writer(writer) => writer.write(bytes)?,
            EncoderSink::Counting(counter) => counter.write(bytes)?,
        };
        if written > bytes.len() {
            return Err(self.reject_overrun());
        }
        // Admission proved the complete slice fits, so every valid partial
        // write fits too. Never advance by bytes the destination did not accept.
        self.written += written;
        Ok(written)
    }
}
impl Write for Encoder<'_> {
    #[inline]
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.rejected_write {
            return Err(self.reject_overrun());
        }
        if self.limit.is_some() {
            return self.write_bounded(bytes);
        }
        match &mut self.sink {
            EncoderSink::Buffer(buffer) => {
                buffer.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            EncoderSink::ByteSink(sink) => {
                sink.write_bytes(bytes);
                Ok(bytes.len())
            }
            EncoderSink::Writer(writer) => writer.write(bytes),
            EncoderSink::Counting(counter) => counter.write(bytes),
        }
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        Encoder::write_all(self, bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        match &mut self.sink {
            EncoderSink::Buffer(_) | EncoderSink::ByteSink(_) | EncoderSink::Counting(_) => Ok(()),
            EncoderSink::Writer(writer) => writer.flush(),
        }
    }
}
