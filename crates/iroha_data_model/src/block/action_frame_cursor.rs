//! Allocation-free cursor for the closed canonical Action payload layout.
//!
//! These are implementation primitives, not proof of a valid Action. All callers
//! must validate the concrete child type and finish every child cursor. The
//! bounds are cumulative for one scanner, including failed attempts; no counter
//! may be reset between lifecycle rows.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScanError {
    Refused,
    Limit,
    Length,
    NonCanonical,
    InvalidValue,
    UnsupportedExecutable,
    UnsupportedAuthority,
    UnsupportedFilter,
    UnsupportedMetadata,
}

impl ScanError {
    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::Refused => "Action scanner range already refused",
            Self::Limit => "Action scanner limit exceeded",
            Self::Length => "Action payload length mismatch",
            Self::NonCanonical => "noncanonical Action payload",
            Self::InvalidValue => "invalid Action field or invariant",
            Self::UnsupportedExecutable => "unsupported Action executable",
            Self::UnsupportedAuthority => "unsupported Action authority",
            Self::UnsupportedFilter => "unsupported Action filter",
            Self::UnsupportedMetadata => "unsupported Action metadata",
        }
    }
}

/// Explicit work limits; these do not fund the owner of the input frame.
#[derive(Clone, Copy, Debug)]
pub struct ActionScannerLimitsV1 {
    /// Maximum byte length of each complete frame admitted by this scanner.
    pub max_frame_bytes: u64,
    /// Maximum cumulative bytes of admitted frames, including nested record frames.
    pub max_total_frame_bytes: u64,
    /// Maximum cumulative number of admitted Action and nested record frames.
    pub max_frames: u64,
    /// Maximum cumulative scanner work units, including complete-frame traversals.
    pub max_work: u64,
    /// Maximum cumulative cursor roots and framed child fields entered.
    pub max_fields: u64,
    /// Maximum cumulative sequence elements, counting each raw-vector byte once.
    pub max_sequence_elements: u64,
    /// Maximum cursor depth, with root depth one and enclosing depth kept for nested frames.
    pub max_depth: u32,
}

#[derive(Debug)]
pub(super) struct Budget {
    pub(super) limits: ActionScannerLimitsV1,
    frames: u64,
    frame_bytes: u64,
    work: u64,
    fields: u64,
    elements: u64,
    refused: bool,
}

impl Budget {
    pub(super) const fn new(limits: ActionScannerLimitsV1) -> Self {
        Self {
            limits,
            frames: 0,
            frame_bytes: 0,
            work: 0,
            fields: 0,
            elements: 0,
            refused: false,
        }
    }

    fn add(value: &mut u64, count: u64, limit: u64) -> Result<(), ScanError> {
        let next = value.checked_add(count).ok_or(ScanError::Limit)?;
        if next > limit {
            return Err(ScanError::Limit);
        }
        *value = next;
        Ok(())
    }

    pub(super) fn begin_frame(&mut self, len: usize) -> Result<(), ScanError> {
        if self.refused {
            return Err(ScanError::Refused);
        }
        let len = u64::try_from(len).map_err(|_| ScanError::Limit)?;
        if len == 0 || len > self.limits.max_frame_bytes {
            return Err(ScanError::Limit);
        }
        Self::add(&mut self.frames, 1, self.limits.max_frames)?;
        Self::add(
            &mut self.frame_bytes,
            len,
            self.limits.max_total_frame_bytes,
        )?;
        // Reserve the complete envelope/checksum traversal before calling Norito.
        self.work(len)
    }

    pub(super) fn refuse(&mut self) {
        self.refused = true;
    }

    pub(super) fn work(&mut self, units: u64) -> Result<(), ScanError> {
        Self::add(&mut self.work, units, self.limits.max_work)
    }

    fn child(&mut self, depth: u32) -> Result<(), ScanError> {
        if depth > self.limits.max_depth {
            return Err(ScanError::Limit);
        }
        Self::add(&mut self.fields, 1, self.limits.max_fields)?;
        self.work(1)
    }

    pub(super) fn elements(&mut self, count: u64) -> Result<(), ScanError> {
        Self::add(&mut self.elements, count, self.limits.max_sequence_elements)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
    depth: u32,
}

impl<'a> Cursor<'a> {
    pub(super) fn root(bytes: &'a [u8], budget: &mut Budget) -> Result<Self, ScanError> {
        budget.child(1)?;
        Ok(Self {
            bytes,
            offset: 0,
            depth: 1,
        })
    }

    /// Enter a framed child of this actual cursor without granting fresh depth.
    /// The caller supplies a payload borrowed from this already bounded field.
    pub(super) fn nested_root(
        self,
        bytes: &'a [u8],
        budget: &mut Budget,
    ) -> Result<Self, ScanError> {
        let depth = self.depth.checked_add(1).ok_or(ScanError::Limit)?;
        budget.child(depth)?;
        Ok(Self {
            bytes,
            offset: 0,
            depth,
        })
    }

    fn take(&mut self, count: usize, budget: &mut Budget) -> Result<&'a [u8], ScanError> {
        let end = self.offset.checked_add(count).ok_or(ScanError::Length)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(ScanError::Length)?;
        budget.work(u64::try_from(count).map_err(|_| ScanError::Limit)?)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn byte(&mut self, budget: &mut Budget) -> Result<u8, ScanError> {
        Ok(self.take(1, budget)?[0])
    }

    pub(super) fn u32(&mut self, budget: &mut Budget) -> Result<u32, ScanError> {
        Ok(u32::from_le_bytes(
            self.take(4, budget)?
                .try_into()
                .map_err(|_| ScanError::Length)?,
        ))
    }

    pub(super) fn u64(&mut self, budget: &mut Budget) -> Result<u64, ScanError> {
        Ok(u64::from_le_bytes(
            self.take(8, budget)?
                .try_into()
                .map_err(|_| ScanError::Length)?,
        ))
    }

    /// Compact-length encoding is fixed by this scanner's exact frame flags.
    /// Reject overflow, continuation beyond ten bytes, and redundant high zeros.
    fn compact_len(&mut self, budget: &mut Budget) -> Result<usize, ScanError> {
        let mut value = 0u64;
        for index in 0..10u32 {
            let byte = self.byte(budget)?;
            let part = byte & 0x7f;
            if index == 9 && (part > 1 || byte & 0x80 != 0) {
                return Err(ScanError::NonCanonical);
            }
            value |= u64::from(part) << (7 * index);
            if byte & 0x80 == 0 {
                if index > 0 && part == 0 {
                    return Err(ScanError::NonCanonical);
                }
                return usize::try_from(value).map_err(|_| ScanError::Limit);
            }
        }
        Err(ScanError::NonCanonical)
    }

    pub(super) fn field(&mut self, budget: &mut Budget) -> Result<Self, ScanError> {
        let len = self.compact_len(budget)?;
        let end = self.offset.checked_add(len).ok_or(ScanError::Length)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(ScanError::Length)?;
        let depth = self.depth.checked_add(1).ok_or(ScanError::Limit)?;
        budget.child(depth)?;
        self.offset = end;
        Ok(Self {
            bytes,
            offset: 0,
            depth,
        })
    }

    pub(super) fn option(&mut self, budget: &mut Budget) -> Result<Option<Self>, ScanError> {
        match self.byte(budget)? {
            0 => Ok(None),
            1 => self.field(budget).map(Some),
            _ => Err(ScanError::InvalidValue),
        }
    }

    /// Vec<u8> has a fixed eight-byte count and raw bytes, unlike Vec<T>.
    pub(super) fn byte_vector(&mut self, budget: &mut Budget) -> Result<&'a [u8], ScanError> {
        let count = self.u64(budget)?;
        budget.elements(count)?;
        let count = usize::try_from(count).map_err(|_| ScanError::Limit)?;
        self.take(count, budget)
    }

    pub(super) fn string(&mut self, budget: &mut Budget) -> Result<&'a str, ScanError> {
        self.bounded_string(usize::MAX, budget)
    }

    /// Enforce a type's fixed protocol byte ceiling before touching its text.
    /// A larger caller work/frame limit must not expand the accepted wire domain.
    pub(super) fn bounded_string(
        &mut self,
        max_bytes: usize,
        budget: &mut Budget,
    ) -> Result<&'a str, ScanError> {
        let count = self.compact_len(budget)?;
        if count > max_bytes {
            return Err(ScanError::InvalidValue);
        }
        let bytes = self.take(count, budget)?;
        // UTF-8 validation is an additional traversal, reserved before it starts.
        budget.work(u64::try_from(count).map_err(|_| ScanError::Limit)?)?;
        core::str::from_utf8(bytes).map_err(|_| ScanError::InvalidValue)
    }

    pub(super) fn finish(self) -> Result<(), ScanError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ScanError::Length)
        }
    }

    pub(super) fn full_bytes(self) -> &'a [u8] {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RetryPolicy {
    pub(super) max_retries: u32,
    pub(super) retry_after_ms: u64,
}

pub(super) fn repeats(mut value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    match value.u32(budget)? {
        0 => (),
        1 => {
            let mut count = value.field(budget)?;
            let _ = count.u32(budget)?;
            count.finish()?;
        }
        _ => return Err(ScanError::InvalidValue),
    }
    value.finish()
}

pub(super) fn retry_policy(
    mut value: Cursor<'_>,
    budget: &mut Budget,
) -> Result<Option<RetryPolicy>, ScanError> {
    let result = match value.option(budget)? {
        None => None,
        Some(mut policy) => {
            let mut count = policy.field(budget)?;
            let max_retries = count.u32(budget)?;
            count.finish()?;
            let mut delay = policy.field(budget)?;
            let retry_after_ms = delay.u64(budget)?;
            delay.finish()?;
            policy.finish()?;
            if max_retries == 0 || retry_after_ms == 0 {
                return Err(ScanError::InvalidValue);
            }
            Some(RetryPolicy {
                max_retries,
                retry_after_ms,
            })
        }
    };
    value.finish()?;
    Ok(result)
}

/// True means Schedule, false means `PreCommit`. No storage kind is supplied by the caller.
pub(super) fn time_filter(mut value: Cursor<'_>, budget: &mut Budget) -> Result<bool, ScanError> {
    let mut execution = value.field(budget)?;
    let scheduled = match execution.u32(budget)? {
        0 => false,
        1 => {
            let mut schedule = execution.field(budget)?;
            let mut start = schedule.field(budget)?;
            let _ = start.u64(budget)?;
            start.finish()?;
            let mut period = schedule.field(budget)?;
            if let Some(mut period_value) = period.option(budget)? {
                let _ = period_value.u64(budget)?;
                period_value.finish()?;
            }
            period.finish()?;
            schedule.finish()?;
            true
        }
        _ => return Err(ScanError::InvalidValue),
    };
    execution.finish()?;
    value.finish()?;
    Ok(scheduled)
}
