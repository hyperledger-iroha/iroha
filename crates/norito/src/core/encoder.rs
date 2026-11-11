//! Serialization destinations used by the Norito core codec.
use super::{
    ByteSink, Error,
    encode_writers::{ExactSliceWriter, LengthCountingWriter},
};
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
    /// Compares every byte with an expected encoding instead of storing it.
    Compare(Comparison<'a>),
}
/// Expected bytes under comparison and the innermost length they declared.
struct Comparison<'a> {
    exact: ExactSliceWriter<'a>,
    /// End, in exact-scope coordinates, of the innermost child whose length
    /// was read from the expected bytes. Writing past it is a byte mismatch,
    /// whereas writing past a measured bound is the serializer's own error.
    claimed_end: Option<usize>,
}
/// Width of a fixed little-endian `u64` length prefix.
const FIXED_PREFIX_LEN: usize = 8;

#[inline]
fn reserve_prefix(buffer: &mut Vec<u8>, compact: bool) {
    if compact {
        buffer.push(0);
    } else {
        buffer.extend_from_slice(&[0; FIXED_PREFIX_LEN]);
    }
}

/// Fill the prefix reserved at `prefix_at` with the length of the bytes after it.
///
/// A compact prefix reserves one byte, which covers every payload shorter than
/// 128 bytes. A longer payload moves up once to admit its wider varint.
#[inline]
fn patch_prefix(
    buffer: &mut Vec<u8>,
    prefix_at: usize,
    reserved: usize,
    compact: bool,
) -> Result<(), Error> {
    let body_at = prefix_at
        .checked_add(reserved)
        .filter(|body_at| *body_at <= buffer.len())
        .ok_or_else(|| Error::LengthMismatch)?;
    let length = buffer.len() - body_at;
    if compact && length < 0x80 {
        buffer[prefix_at] = length as u8;
        return Ok(());
    }
    patch_wide_prefix(buffer, prefix_at, body_at, compact)
}
fn patch_wide_prefix(
    buffer: &mut Vec<u8>,
    prefix_at: usize,
    body_at: usize,
    compact: bool,
) -> Result<(), Error> {
    let body_end = buffer.len();
    let length = u64::try_from(body_end - body_at).map_err(|_| Error::LengthMismatch)?;
    if !compact {
        buffer[prefix_at..body_at].copy_from_slice(&length.to_le_bytes());
        return Ok(());
    }
    let mut varint = [0_u8; super::MAX_VARINT_BYTES];
    let used = super::encode_varint(length, &mut varint);
    let widened = used - (body_at - prefix_at);
    buffer.resize(body_end + widened, 0);
    buffer.copy_within(body_at..body_end, body_at + widened);
    buffer[prefix_at..prefix_at + used].copy_from_slice(&varint[..used]);
    Ok(())
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
// A child whose length the expected bytes declared: restores the enclosing
// bound and claimed end on return and unwind.
struct ClaimedLengthScope<'scope, 'sink> {
    encoder: &'scope mut Encoder<'sink>,
    previous_limit: Option<usize>,
    previous_claimed_end: Option<usize>,
}
impl Drop for ClaimedLengthScope<'_, '_> {
    fn drop(&mut self) {
        self.encoder.limit = self.previous_limit;
        if let EncoderSink::Compare(comparison) = &mut self.encoder.sink {
            comparison.claimed_end = self.previous_claimed_end;
        }
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

    /// Run `body` against an encoder that compares with `expected`.
    ///
    /// Returns the body's result and the comparison, whose verdict is that of
    /// serializing into an [`ExactSliceWriter`] over `expected`.
    pub(super) fn compare_against<R>(
        expected: &'a [u8],
        body: impl FnOnce(&mut Encoder<'a>) -> R,
    ) -> (R, ExactSliceWriter<'a>) {
        let mut encoder = Self::with_sink(EncoderSink::Compare(Comparison {
            exact: ExactSliceWriter::new(expected),
            claimed_end: None,
        }));
        let result = body(&mut encoder);
        let exact = match encoder.sink {
            EncoderSink::Compare(comparison) => comparison.exact,
            _ => {
                let mut exact = ExactSliceWriter::new(expected);
                exact.mark_mismatch();
                exact
            }
        };
        (result, exact)
    }

    /// Whether this encoder compares with expected bytes, outside any bounded
    /// work or count scope, with no overrun observed.
    pub(super) fn compares_in_place(&self) -> bool {
        matches!(self.sink, EncoderSink::Compare(_))
            && !self.rejected_write
            && !super::encode_work::thread_accounting_active()
    }

    /// The `len` expected bytes that follow the next `skip`, when present.
    pub(super) fn compare_upcoming(&self, skip: usize, len: usize) -> Option<&[u8]> {
        let EncoderSink::Compare(comparison) = &self.sink else {
            return None;
        };
        let start = skip;
        comparison
            .exact
            .remaining()
            .get(start..start.checked_add(len)?)
    }

    /// Record a byte difference found without writing the differing bytes.
    pub(super) fn mark_compare_mismatch(&mut self) {
        if let EncoderSink::Compare(comparison) = &mut self.sink {
            comparison.exact.mark_mismatch();
        }
    }

    /// The owned buffer this encoder appends to, when bytes may be placed in
    /// it after the fact: no exact-length scope bounds the writes, no overrun
    /// was observed, and no bounded work or count scope fixes their charges.
    pub(super) fn in_place_buffer(&mut self) -> Option<&mut Vec<u8>> {
        if self.limit.is_some()
            || self.rejected_write
            || super::encode_work::thread_accounting_active()
        {
            return None;
        }
        match &mut self.sink {
            EncoderSink::Buffer(buffer) => Some(buffer),
            EncoderSink::ByteSink(sink) => Some(&mut sink.buf),
            EncoderSink::Writer(_) | EncoderSink::Counting(_) | EncoderSink::Compare(_) => None,
        }
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
            if let EncoderSink::Compare(comparison) = &mut self.sink
                && comparison.claimed_end.is_some_and(|claimed| end > claimed)
            {
                // The child outgrew the length the expected bytes declared.
                comparison.exact.mark_mismatch();
            }
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
            .ok_or_else(|| Error::LengthMismatch)
    }

    /// Frame `value` as `[len][payload]` from the bytes it actually writes.
    ///
    /// A buffer destination reserves the prefix, serializes the child once and
    /// patches the prefix with the written length. A counting destination adds
    /// the child's bytes to its own total and then the prefix they require.
    /// Neither measures the child in a separate pass, so nesting depth no
    /// longer multiplies the work, and a length can never disagree with the
    /// bytes it frames. The emitted bytes equal the measured path's.
    ///
    /// Returns `Ok(false)` without writing when the caller must use the
    /// measured path: a streaming destination needs the length before the
    /// payload, an exact-length scope bounds each write as it happens, and a
    /// bounded work or count scope fixes the charge of every measurement.
    ///
    /// # Errors
    /// Propagates the child's error. The destination then holds the reserved
    /// prefix and a payload prefix, which the caller must discard.
    pub(super) fn write_framed_child(
        &mut self,
        value: &dyn super::SerializePayload,
        prefix: super::LengthPrefix,
    ) -> Result<bool, Error> {
        if self.rejected_write || super::encode_work::thread_accounting_active() {
            return Ok(false);
        }
        let compact = match prefix {
            super::LengthPrefix::Layout(flags) => super::compact_len_enabled_for_flags(flags),
            super::LengthPrefix::Fixed => false,
        };
        if matches!(self.sink, EncoderSink::Compare(_)) {
            self.compare_framed_child(value, compact)?;
            return Ok(true);
        }
        if self.limit.is_some() {
            return Ok(false);
        }
        let reserved = if compact { 1 } else { FIXED_PREFIX_LEN };
        let prefix_at = match &mut self.sink {
            EncoderSink::Buffer(buffer) => {
                let at = buffer.len();
                reserve_prefix(buffer, compact);
                at
            }
            EncoderSink::ByteSink(sink) => {
                let at = sink.buf.len();
                sink.ensure_capacity(reserved);
                reserve_prefix(&mut sink.buf, compact);
                at
            }
            EncoderSink::Counting(counter) => counter.len,
            EncoderSink::Writer(_) | EncoderSink::Compare(_) => return Ok(false),
        };
        value.serialize(self)?;
        if self.rejected_write {
            return Err(Error::LengthMismatch);
        }
        match &mut self.sink {
            EncoderSink::Buffer(buffer) => patch_prefix(buffer, prefix_at, reserved, compact)?,
            EncoderSink::ByteSink(sink) => {
                patch_prefix(&mut sink.buf, prefix_at, reserved, compact)?;
            }
            EncoderSink::Counting(counter) => {
                let length = counter
                    .len
                    .checked_sub(prefix_at)
                    .ok_or_else(|| Error::LengthMismatch)?;
                let prefix_len = if compact {
                    super::varint_encoded_len(
                        u64::try_from(length).map_err(|_| Error::LengthMismatch)?,
                    )
                } else {
                    FIXED_PREFIX_LEN
                };
                counter.add(prefix_len)?;
            }
            EncoderSink::Writer(_) | EncoderSink::Compare(_) => return Err(Error::LengthMismatch),
        }
        if compact {
            super::note_compact_len_emitted();
        }
        Ok(true)
    }

    /// Compare `[len][payload]` with the expected bytes in one pass.
    ///
    /// The expected bytes declare the length. The verdict equals that of the
    /// measured path: its output matches exactly when the declared prefix is
    /// the canonical prefix of a length `L`, the child writes exactly `L`
    /// bytes, and those bytes match. Canonical varint decoding admits only the
    /// shortest encoding, so a matching prefix is byte-identical to the one
    /// the measured path would write.
    ///
    /// # Errors
    /// Reports a difference as [`Error::NonCanonicalEncoding`] with the
    /// mismatch recorded; propagates the child's own errors.
    fn compare_framed_child(
        &mut self,
        value: &dyn super::SerializePayload,
        compact: bool,
    ) -> Result<(), Error> {
        let EncoderSink::Compare(comparison) = &mut self.sink else {
            return Err(Error::LengthMismatch);
        };
        let remaining = comparison.exact.remaining();
        let declared = if compact {
            super::decode_varint_from_slice(remaining).ok()
        } else {
            remaining
                .first_chunk::<FIXED_PREFIX_LEN>()
                .map(|prefix| (u64::from_le_bytes(*prefix), FIXED_PREFIX_LEN))
        };
        let Some((length, prefix_len)) =
            declared.and_then(|(length, used)| Some((usize::try_from(length).ok()?, used)))
        else {
            // No canonical prefix of any length is there.
            comparison.exact.mark_mismatch();
            return Err(Error::NonCanonicalEncoding);
        };
        // The declared prefix stands for the bytes the measured path writes.
        if self.limit.is_some() {
            self.written = self.admitted_end(prefix_len)?;
        }
        if let EncoderSink::Compare(comparison) = &mut self.sink {
            comparison.exact.skip_matched(prefix_len);
        }
        if compact {
            super::note_compact_len_emitted();
        }
        let previous_limit = self.limit;
        if previous_limit.is_none() {
            // Each outermost child starts a fresh relative coordinate system.
            self.written = 0;
        }
        let end = match self.written.checked_add(length) {
            Some(end) if previous_limit.is_none_or(|limit| end <= limit) => end,
            // The declared child extends past its enclosing value.
            _ => {
                self.mark_compare_mismatch();
                return Err(Error::NonCanonicalEncoding);
            }
        };
        let previous_claimed_end = match &mut self.sink {
            EncoderSink::Compare(comparison) => comparison.claimed_end.replace(end),
            _ => None,
        };
        self.limit = Some(end);
        let scope = ClaimedLengthScope {
            encoder: self,
            previous_limit,
            previous_claimed_end,
        };
        let result = value.serialize(scope.encoder);
        if scope.encoder.rejected_write {
            return Err(Error::LengthMismatch);
        }
        result?;
        if scope.encoder.written != end {
            // The child is shorter than the declared length.
            scope.encoder.mark_compare_mismatch();
            return Err(Error::NonCanonicalEncoding);
        }
        Ok(())
    }

    // Only codec helpers which have themselves measured this child may use this.
    // Arbitrary writers, including checksum/comparison writers over io::sink(),
    // must still receive every actual byte. Counting is a property of this
    // destination, never a process/thread mode inherited by unrelated encoders.
    #[inline]
    pub(super) fn count_measured_bytes(&mut self, length: usize) -> Result<bool, Error> {
        super::encode_work::charge(0)?;
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
        super::encode_work::charge(bytes.len())?;
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
            EncoderSink::Compare(comparison) => {
                comparison.exact.compare(bytes);
                Ok(())
            }
        }
    }
    /// Write a fixed-width value, such as a little-endian scalar.
    ///
    /// The constant width lets a buffer destination store the bytes inline
    /// instead of calling a general copy for a few bytes. Accounting, bounds
    /// and every other destination behave exactly as in [`Self::write_all`].
    #[inline]
    pub fn write_array<const N: usize>(&mut self, bytes: &[u8; N]) -> io::Result<()> {
        if self.limit.is_none() && !self.rejected_write && !super::encode_work::any_bounded_scope()
        {
            match &mut self.sink {
                EncoderSink::Buffer(buffer) => {
                    buffer.extend_from_slice(bytes);
                    return Ok(());
                }
                EncoderSink::ByteSink(sink) => {
                    sink.ensure_capacity(N);
                    sink.buf.extend_from_slice(bytes);
                    return Ok(());
                }
                EncoderSink::Writer(_) | EncoderSink::Counting(_) | EncoderSink::Compare(_) => {}
            }
        }
        self.write_all(bytes)
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
            EncoderSink::Compare(comparison) => {
                comparison.exact.compare(bytes);
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
            EncoderSink::Compare(comparison) => {
                comparison.exact.compare(bytes);
                bytes.len()
            }
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
        super::encode_work::charge(bytes.len())?;
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
            EncoderSink::Compare(comparison) => {
                comparison.exact.compare(bytes);
                Ok(bytes.len())
            }
        }
    }
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        Encoder::write_all(self, bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        super::encode_work::charge(0)?;
        match &mut self.sink {
            EncoderSink::Buffer(_)
            | EncoderSink::ByteSink(_)
            | EncoderSink::Counting(_)
            | EncoderSink::Compare(_) => Ok(()),
            EncoderSink::Writer(writer) => writer.flush(),
        }
    }
}
