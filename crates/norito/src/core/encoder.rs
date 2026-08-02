//! Serialization destinations used by the Norito core codec.

use std::io::{self, Write};

use super::ByteSink;

/// Non-generic destination used by [`super::NoritoSerialize`] implementations.
///
/// Erasing the concrete [`Write`] type at the outer serialization boundary
/// prevents every model type from being monomorphized once per destination.
/// A dedicated buffer path keeps the common temporary-field and bare-payload
/// encoders allocation-free beyond the buffer they already own.
pub struct Encoder<'a> {
    sink: EncoderSink<'a>,
}

enum EncoderSink<'a> {
    Buffer(&'a mut Vec<u8>),
    ByteSink(&'a mut ByteSink),
    Writer(&'a mut dyn Write),
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
    /// This constructor is primarily used by generated serializers when they
    /// stage a length-delimited field. Its sink representation remains an
    /// implementation detail so callers cannot depend on dispatch strategy.
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
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        Encoder::write_all(self, bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.sink {
            EncoderSink::Buffer(_) | EncoderSink::ByteSink(_) => Ok(()),
            EncoderSink::Writer(writer) => writer.flush(),
        }
    }
}
