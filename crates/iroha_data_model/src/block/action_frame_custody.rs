//! Immutable physical custody of supplied or physically serialized Action-frame bytes.
//!
//! This owner proves only exact copying and retention of its original prepaid
//! backing. The input remains untrusted; neither this type nor its constructors
//! authenticates an Action, its source, or a lifecycle statement. Source admission,
//! serializer scratch and enclosing journal/control storage need their own owners.

use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};

/// One complete supplied byte slice and its inseparable original backing charge.
///
/// There is no Clone, mutable access, Vec extraction or normalization.
/// Moving this owner keeps the same allocation and pool charge. Borrowing cannot
/// outlive the owner. The underlying ChargedBuffer destroys its byte allocation
/// before returning its credits; dropping an unpublished owner is its rollback.
/// A consumer may separately copy a borrowed slice, but this owner funds no copy.
///
/// An enclosing heap shell, journal slot, input source or scanner scratch is not
/// funded by this type. Those remain explicit original-owner obligations.
pub struct RetainedActionFrameBytesV1 {
    bytes: ChargedBuffer<u8>,
}

impl RetainedActionFrameBytesV1 {
    /// Called only after the scanner admits frame/count and copy-work bounds.
    /// ChargedBuffer checks Layout::array::<u8>(len) and acquires the original
    /// pool charge before its sole allocation; no source byte is copied first.
    pub(super) fn try_copy(
        source: &[u8],
        pool: &AllocationBudget,
    ) -> Result<Self, ChargedBufferError> {
        let mut bytes = ChargedBuffer::new(source.len(), pool)?;
        // The exact fixed capacity was admitted above. Each original u8 moves
        // into the next slot without cloning, staging or growing that backing.
        // No fallible operation follows the first copy in this constructor.
        for &byte in source {
            bytes.push_reserved(byte);
        }
        Ok(Self { bytes })
    }

    /// Borrow every original byte, without implying successful validation.
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    /// Observe the exact retained backing charge's original finite pool.
    pub fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.bytes.belongs_to(pool)
    }

    /// Exact requested backing size retained until this owner is destroyed.
    /// The allocation is a fixed u8 array; no nested payload allocations exist.
    pub fn backing_bytes(&self) -> usize {
        self.bytes.capacity()
    }
}

impl core::fmt::Debug for RetainedActionFrameBytesV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Diagnostic formatting does not disclose or copy the supplied frame.
        f.debug_struct("RetainedActionFrameBytesV1")
            .field("bytes", &self.bytes.as_slice().len())
            .finish_non_exhaustive()
    }
}

/// Original first failure while preparing physical bytes. This does not include
/// semantic validation: completed output remains unvalidated Action-frame bytes.
#[derive(Debug)]
pub enum ActionFrameProductionFailureV1 {
    /// Exact original pool admission or allocation failure, before output.
    Allocation(ChargedBufferError),
    /// Original bounded codec refusal, including source and destination errors.
    Codec(norito::core::BoundedEncodeWorkError),
}

struct ChargedFrameWriter<'pool> {
    pool: &'pool AllocationBudget,
    bytes: Option<ChargedBuffer<u8>>,
    allocation_failure: Option<ChargedBufferError>,
}
impl ChargedFrameWriter<'_> {
    fn prepare(&mut self, frame_len: usize) -> Result<(), norito::Error> {
        if self.bytes.is_some() || self.allocation_failure.is_some() {
            return Err(norito::Error::LengthMismatch);
        }
        match ChargedBuffer::new(frame_len, self.pool) {
            Ok(bytes) => {
                self.bytes = Some(bytes);
                Ok(())
            }
            Err(error) => {
                self.allocation_failure = Some(error);
                Err(norito::Error::LengthMismatch)
            }
        }
    }
}
impl std::io::Write for ChargedFrameWriter<'_> {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        let bytes = self.bytes.as_mut().ok_or(std::io::ErrorKind::InvalidData)?;
        let end = bytes
            .as_slice()
            .len()
            .checked_add(input.len())
            .ok_or(std::io::ErrorKind::InvalidData)?;
        if end > bytes.capacity() {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        for &byte in input {
            bytes.push_reserved(byte);
        }
        Ok(input.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl RetainedActionFrameBytesV1 {
    /// Encode a caller-supplied, unvalidated payload with Action framing without an
    /// owned Action graph, temporary Vec or a second frame copy.
    ///
    /// This is a low-level physical preparation seam, not source admission:
    /// the caller must prove the borrowed six fields are the original Action
    /// and separately fund/bound every serializer-internal source/scratch path.
    /// The finite codec scope cannot police arbitrary work before codec calls.
    ///
    /// Actual Action selects schema and padding. The first bounded checksum
    /// pass determines the full size. Its measured hook acquires the ORIGINAL
    /// pool charge before the only allocation and before any output byte.
    /// Both frame passes share one allowance. Success seals that same backing;
    /// any codec failure drops partial output and charge together. The scanner
    /// must retain this owner on later failure rather than copying its bytes.
    #[doc(hidden)]
    pub fn try_encode_unvalidated_payload(
        payload: &dyn norito::core::SerializePayload,
        pool: &AllocationBudget,
        limits: norito::core::EncodeWorkLimits,
    ) -> Result<Self, ActionFrameProductionFailureV1> {
        let mut writer = ChargedFrameWriter {
            pool,
            bytes: None,
            allocation_failure: None,
        };
        let result = norito::core::write_canonical_payload_bounded::<
            crate::trigger::action::Action,
            _,
            _,
            _,
        >(payload, &mut writer, limits, |writer, frame_len| {
            writer.prepare(frame_len)
        });
        if let Some(error) = writer.allocation_failure.take() {
            return Err(ActionFrameProductionFailureV1::Allocation(error));
        }
        result.map_err(ActionFrameProductionFailureV1::Codec)?;
        let bytes = writer.bytes.take().ok_or_else(|| {
            ActionFrameProductionFailureV1::Codec(norito::core::BoundedEncodeWorkError::Codec(
                norito::Error::LengthMismatch,
            ))
        })?;
        if bytes.as_slice().len() != bytes.capacity() {
            return Err(ActionFrameProductionFailureV1::Codec(
                norito::core::BoundedEncodeWorkError::Codec(norito::Error::LengthMismatch),
            ));
        }
        Ok(Self { bytes })
    }
}

#[cfg(test)]
#[path = "action_frame_production_tests.rs"]
mod production_tests;
