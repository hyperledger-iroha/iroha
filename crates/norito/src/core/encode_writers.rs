//! Allocation-free writers used by count-first Norito encoding.
use std::{
    cell::Cell,
    io::{self, Write},
};
#[derive(Clone, Copy)]
struct CountBudget {
    remaining: usize,
    rejected: bool,
}
thread_local! {
    static ACTIVE_COUNT_BUDGET: Cell<Option<CountBudget>> = const { Cell::new(None) };
}
/// A caller's finite count allowance, shared by all nested measurement writers.
pub(super) struct CountBudgetGuard {
    previous: Option<CountBudget>,
    initial: usize,
    _registered: super::encode_work::BoundedScope,
}
impl CountBudgetGuard {
    pub(super) fn enter(maximum: usize) -> Self {
        // Registered before the budget it guards, so no charge can miss it.
        let _registered = super::encode_work::BoundedScope::register();
        let previous = ACTIVE_COUNT_BUDGET.get();
        let initial = previous.map_or(maximum, |budget| maximum.min(budget.remaining));
        ACTIVE_COUNT_BUDGET.set(Some(CountBudget {
            remaining: initial,
            rejected: previous.is_some_and(|budget| budget.rejected),
        }));
        Self {
            previous,
            initial,
            _registered,
        }
    }
    pub(super) fn check(&self) -> Result<(), super::Error> {
        if ACTIVE_COUNT_BUDGET
            .get()
            .is_some_and(|budget| budget.rejected)
        {
            Err(super::Error::LengthMismatch)
        } else {
            Ok(())
        }
    }
}
impl Drop for CountBudgetGuard {
    fn drop(&mut self) {
        let current = ACTIVE_COUNT_BUDGET
            .get()
            .expect("original count budget scope");
        let previous = self.previous.map(|mut previous| {
            previous.remaining -= self.initial - current.remaining;
            previous.rejected |= current.rejected;
            previous
        });
        ACTIVE_COUNT_BUDGET.set(previous);
    }
}
/// True when this thread counts under a finite allowance.
pub(super) fn count_budget_active() -> bool {
    ACTIVE_COUNT_BUDGET.get().is_some()
}
#[inline]
pub(super) fn charge_counted_bytes(bytes: usize) -> io::Result<()> {
    if !super::encode_work::any_bounded_scope() {
        return Ok(());
    }
    charge_counted_bytes_bounded(bytes)
}
fn charge_counted_bytes_bounded(bytes: usize) -> io::Result<()> {
    let Some(mut budget) = ACTIVE_COUNT_BUDGET.get() else {
        return Ok(());
    };
    if budget.rejected || bytes > budget.remaining {
        budget.rejected = true;
        ACTIVE_COUNT_BUDGET.set(Some(budget));
        // A count refusal precedes output admission, so its diagnostic must not
        // allocate either. Preserve this original I/O error through the codec;
        // an ErrorKind alone cannot authenticate an error for later rewriting.
        return Err(io::ErrorKind::Other.into());
    }
    budget.remaining -= bytes;
    ACTIVE_COUNT_BUDGET.set(Some(budget));
    Ok(())
}
/// Writer which counts bytes without retaining them.
#[derive(Default)]
pub(super) struct LengthCountingWriter {
    pub(super) len: usize,
    overflowed: bool,
}
impl LengthCountingWriter {
    pub(super) fn add(&mut self, bytes: usize) -> io::Result<()> {
        if !self.overflowed {
            if let Some(length) = self.len.checked_add(bytes) {
                self.len = length;
                return Ok(());
            }
            self.overflowed = true;
        }
        Err(io::ErrorKind::InvalidData.into())
    }

    pub(super) fn finish(self) -> Result<usize, super::Error> {
        if self.overflowed {
            Err(super::Error::LengthMismatch)
        } else {
            Ok(self.len)
        }
    }
}
impl Write for LengthCountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        charge_counted_bytes(buf.len())?;
        self.add(buf.len())?;
        Ok(buf.len())
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        charge_counted_bytes(buf.len())?;
        self.add(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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
            .ok_or(io::ErrorKind::InvalidData)?;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Writer that compares serialized bytes directly with an expected slice.
///
/// Mismatches are sticky while writes report full consumption, allowing the
/// serializer to finish without allocating a comparison buffer.
pub(crate) struct ExactSliceWriter<'a> {
    expected: &'a [u8],
    offset: usize,
    mismatched: bool,
}

impl<'a> ExactSliceWriter<'a> {
    pub(crate) const fn new(expected: &'a [u8]) -> Self {
        Self {
            expected,
            offset: 0,
            mismatched: false,
        }
    }

    pub(crate) const fn matched_len(&self) -> usize {
        self.offset
    }

    pub(crate) const fn mismatched(&self) -> bool {
        self.mismatched
    }

    pub(crate) const fn is_complete(&self) -> bool {
        !self.mismatched && self.offset == self.expected.len()
    }

    /// The expected bytes not yet compared.
    pub(super) fn remaining(&self) -> &'a [u8] {
        self.expected.get(self.offset..).unwrap_or_default()
    }

    /// Record a difference found without writing the differing bytes.
    pub(super) fn mark_mismatch(&mut self) {
        self.mismatched = true;
    }

    /// Advance over `len` bytes already proven equal to the expected bytes.
    pub(super) fn skip_matched(&mut self, len: usize) {
        match self.offset.checked_add(len) {
            Some(end) if end <= self.expected.len() => self.offset = end,
            _ => {
                self.mismatched = true;
                self.offset = self.expected.len();
            }
        }
    }

    /// Compare `bytes` with the next expected bytes. A difference, including
    /// running past the expected end, is recorded and never reported as an
    /// I/O error, so the serializer always runs to completion.
    pub(super) fn compare(&mut self, bytes: &[u8]) {
        let Some(end) = self.offset.checked_add(bytes.len()) else {
            self.mismatched = true;
            return;
        };
        let Some(expected) = self.expected.get(self.offset..end) else {
            self.mismatched = true;
            self.offset = self.expected.len();
            return;
        };
        if expected != bytes {
            self.mismatched = true;
        }
        self.offset = end;
    }
}

impl Write for ExactSliceWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.compare(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_inline_overflow(error: io::Error) {
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.get_ref().is_none(), "no boxed diagnostic payload");
        assert!(error.raw_os_error().is_none(), "no substituted OS error");
    }

    #[test]
    fn count_writer_checked_add_overflow_retains_inline_error_and_sticky_state() {
        let mut counted = LengthCountingWriter {
            len: usize::MAX,
            overflowed: false,
        };
        assert_inline_overflow(counted.add(1).unwrap_err());
        assert_eq!(counted.len, usize::MAX);
        assert_inline_overflow(counted.add(0).unwrap_err());
        assert!(matches!(
            counted.finish(),
            Err(super::super::Error::LengthMismatch)
        ));
    }

    #[test]
    fn exact_writer_rejects_oversized_report_without_boxed_overflow_diagnostic() {
        // A conforming Write cannot reach this checked-add overflow because the
        // requested slice was already bounded. Exercise the actual defensive
        // branch with an adversarial downstream writer, without allocating data.
        struct OversizedReport;
        impl Write for OversizedReport {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Ok(usize::MAX)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut original = OversizedReport;
        let mut writer = ExactLengthWriter::new(&mut original, 2);
        writer.len = 1;
        assert_inline_overflow(writer.write(&[7]).unwrap_err());
        assert_eq!(
            writer.written_len(),
            1,
            "overflow never publishes an invented length"
        );
    }
}
