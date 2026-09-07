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
}
enum EncoderSink<'a> {
    Buffer(&'a mut Vec<u8>),
    ByteSink(&'a mut ByteSink),
    Writer(&'a mut dyn Write),
    Counting(&'a mut LengthCountingWriter),
}
impl<'a> Encoder<'a> {
    /// Create an encoder over an arbitrary byte writer.
    pub fn new(writer: &'a mut dyn Write) -> Self {
        Self {
            sink: EncoderSink::Writer(writer),
        }
    }
    /// Create an encoder that appends directly to `buffer`.
    ///
    /// Use this when the caller needs owned payload bytes. Length-prefixed fields
    /// stream through their destination without staging in a separate buffer.
    /// The sink representation remains an implementation detail.
    #[doc(hidden)]
    pub fn for_buffer(buffer: &'a mut Vec<u8>) -> Self {
        Self {
            sink: EncoderSink::Buffer(buffer),
        }
    }
    pub(super) fn for_byte_sink(sink: &'a mut ByteSink) -> Self {
        Self {
            sink: EncoderSink::ByteSink(sink),
        }
    }
    pub(super) fn for_counting(counter: &'a mut LengthCountingWriter) -> Self {
        Self {
            sink: EncoderSink::Counting(counter),
        }
    }

    pub(super) fn is_counting(&self) -> bool {
        matches!(self.sink, EncoderSink::Counting(_))
    }

    // Only codec helpers which have themselves measured this child may use this.
    // Arbitrary writers, including checksum/comparison writers over io::sink(),
    // must still receive every actual byte. Counting is a property of this
    // destination, never a process/thread mode inherited by unrelated encoders.
    pub(super) fn count_measured_bytes(&mut self, length: usize) -> Result<bool, Error> {
        if let EncoderSink::Counting(counter) = &mut self.sink {
            counter.add(length)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    /// Write an entire byte slice to the serialization destination.
    ///
    /// This inherent operation keeps generated implementations independent of
    /// whether [`Write`] happens to be imported at the derive site.
    pub fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
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
}
impl Write for Encoder<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
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
