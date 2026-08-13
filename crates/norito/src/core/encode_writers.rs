//! Allocation-free writers used by count-first Norito encoding.
use std::io::{self, Write};
/// Writer which counts bytes without retaining them.
#[derive(Default)]
pub(super) struct LengthCountingWriter {
    pub(super) len: usize,
}
impl Write for LengthCountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.len = self
            .len
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("Norito encoded length overflow"))?;
        Ok(buf.len())
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.len = self
            .len
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("Norito encoded length overflow"))?;
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Writer which forwards bytes while recording their length.
pub(super) struct CountingWriter<'a, W> {
    pub(super) inner: &'a mut W,
    pub(super) len: usize,
}
impl<W: Write> Write for CountingWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.len = self
            .len
            .checked_add(written)
            .ok_or_else(|| io::Error::other("Norito encoded length overflow"))?;
        Ok(written)
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.inner.write_all(buf)?;
        self.len = self
            .len
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("Norito encoded length overflow"))?;
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
/// Writer which refuses a counted second-pass overrun before forwarding it.
pub(super) struct ExactLengthWriter<'a, W> {
    inner: &'a mut W,
    len: usize,
    expected_len: usize,
    rejected_write: bool,
}
impl<'a, W> ExactLengthWriter<'a, W> {
    pub(super) fn new(inner: &'a mut W, expected_len: usize) -> Self {
        Self {
            inner,
            len: 0,
            expected_len,
            rejected_write: false,
        }
    }
    pub(super) const fn written_len(&self) -> usize {
        self.len
    }
    pub(super) const fn rejected_write(&self) -> bool {
        self.rejected_write
    }
    fn reject_overrun(&mut self) -> io::Error {
        self.rejected_write = true;
        io::ErrorKind::InvalidData.into()
    }
}
impl<W: Write> Write for ExactLengthWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.rejected_write {
            return Err(self.reject_overrun());
        }
        let Some(next) = self.len.checked_add(buf.len()) else {
            return Err(self.reject_overrun());
        };
        if next > self.expected_len {
            return Err(self.reject_overrun());
        }
        let written = self.inner.write(buf)?;
        self.len = self
            .len
            .checked_add(written)
            .ok_or_else(|| io::Error::other("Norito encoded length overflow"))?;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
