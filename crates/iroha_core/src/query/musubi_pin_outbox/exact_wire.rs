//! UNLINKED DRAFT: original Pending Check plus its directly produced fixed-V1 signed wire.
//!
//! TODO: Link only after the data-model counted writer replaces the ordinary helper and both
//! actual serializer passes have original-owner scratch admission. This code owns the output
//! backing only. Existing SignedTransaction/BoundNativeCheck strings, entry Vec, preparation,
//! signing, decode context, native filesystem and complete journal inventory remain separate
//! unresolved owners; this module must not make the stock daemon path operational.

use super::{MusubiPinOutboxCheckErrorV1, PendingMusubiPinOutboxCheckV1};
use crate::{execution_attempt::ExecutionDeferred, state::StateReadOnly as _};
use iroha_allocation::{ChargedBuffer, ChargedBufferError};
use iroha_data_model::isi::musubi::MUSUBI_PIN_OUTBOX_EXTERNAL_MAX_BYTES_V1;
use std::{fmt, io, time::Instant};

/// A capture refusal with its exact original local reason, never a transaction verdict.
#[derive(Debug)]
pub enum MusubiPinOutboxWireCaptureErrorV1 {
    /// Original State allocation refusal, including its release owner when one exists.
    Deferred(ExecutionDeferred),
    /// Original codec error; serializer scratch refusal is not relabeled as invalid input.
    Codec(norito::Error),
    /// The original live round refused capture, including expiration without renewal.
    Check(MusubiPinOutboxCheckErrorV1),
    /// Measured exact wire cannot fit the existing native Check envelope ceiling.
    WireLimit {
        /// Actual complete fixed-V1 signed extent.
        measured: usize,
        /// Existing native Check envelope ceiling.
        maximum: usize,
    },
}

impl From<ChargedBufferError> for MusubiPinOutboxWireCaptureErrorV1 {
    fn from(error: ChargedBufferError) -> Self {
        match error {
            ChargedBufferError::Admission(original) => Self::Deferred(original.into()),
            ChargedBufferError::Allocator { .. } => {
                Self::Deferred(ivm::error::ExecutionDeferral::AllocationUnavailable.into())
            }
        }
    }
}

/// The unchanged original pending Check retained on every failed capture attempt.
#[must_use = "retain the original pending Check and deadline after capture refusal"]
pub struct MusubiPinOutboxWireCaptureFailureV1 {
    pending: PendingMusubiPinOutboxCheckV1,
    error: MusubiPinOutboxWireCaptureErrorV1,
}

impl MusubiPinOutboxWireCaptureFailureV1 {
    /// Borrow the exact reason without cloning, formatting or erasing its retry owner.
    #[must_use]
    pub const fn error(&self) -> &MusubiPinOutboxWireCaptureErrorV1 {
        &self.error
    }

    /// Move the same original pending Check back to its owner; the deadline is unchanged.
    #[must_use]
    pub fn into_pending(self) -> PendingMusubiPinOutboxCheckV1 {
        self.pending
    }

    /// Retain both the original pending Check and original refusal for coordinated retry.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        PendingMusubiPinOutboxCheckV1,
        MusubiPinOutboxWireCaptureErrorV1,
    ) {
        (self.pending, self.error)
    }
}

impl fmt::Debug for MusubiPinOutboxWireCaptureFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MusubiPinOutboxWireCaptureFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

/// Exact original signed bytes and live Pending custody; neither authorizes submission.
#[must_use = "retain both original signed wire and live Check custody until durable handoff"]
pub struct CapturedMusubiPinOutboxWireV1 {
    pending: PendingMusubiPinOutboxCheckV1,
    wire: ChargedBuffer<u8>,
}

impl CapturedMusubiPinOutboxWireV1 {
    /// Borrow the complete original SignedTransaction fixed-V1 wire, including authorization.
    #[must_use]
    pub fn exact_wire(&self) -> &[u8] {
        self.wire.as_slice()
    }

    /// Original absolute round deadline; capture and storage never extend it.
    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.pending.deadline()
    }

    /// Transfer the original live custody and actual owned backing to a future journal owner.
    ///
    /// The daemon must independently authenticate its own original State/session/inventory
    /// permit and match the returned buffer's pool. This transfer grants no such permit.
    #[must_use]
    pub fn into_parts(self) -> (PendingMusubiPinOutboxCheckV1, ChargedBuffer<u8>) {
        (self.pending, self.wire)
    }
}

impl PendingMusubiPinOutboxCheckV1 {
    /// Produce exact signed bytes directly in the original State pool while preserving custody.
    ///
    /// There is no caller-supplied budget, deadline, Check challenge or replacement transaction.
    /// Both outcomes move the same Pending instance; no failure invokes a signer or a decoder.
    ///
    /// # Errors
    /// Returns unchanged original Pending custody and the original refusal when admission,
    /// either serialization pass, the wire bound or the unchanged deadline refuses capture.
    pub fn capture_exact_wire_v1(
        self,
    ) -> Result<CapturedMusubiPinOutboxWireV1, MusubiPinOutboxWireCaptureFailureV1> {
        let result = (|| {
            self.prepared
                .round
                .ensure_live()
                .map_err(|error| MusubiPinOutboxWireCaptureErrorV1::Check(error.into()))?;
            // TODO: The plan's real counting pass can allocate concrete serializer scratch.
            // Its output count alone is not preallocation funding for that scratch.
            let plan = self
                .signed_transaction()
                .wire_plan_v1()
                .map_err(MusubiPinOutboxWireCaptureErrorV1::Codec)?;
            let length = plan.wire_length();
            if length > MUSUBI_PIN_OUTBOX_EXTERNAL_MAX_BYTES_V1 {
                return Err(MusubiPinOutboxWireCaptureErrorV1::WireLimit {
                    measured: length,
                    maximum: MUSUBI_PIN_OUTBOX_EXTERNAL_MAX_BYTES_V1,
                });
            }
            let original_budget = self.prepared.state.query_view().execution_budget();
            let mut destination =
                OriginalWireDestination(ChargedBuffer::new(length, &original_budget)?);
            plan.write_to(&mut destination)
                .map_err(MusubiPinOutboxWireCaptureErrorV1::Codec)?;
            self.prepared
                .round
                .ensure_live()
                .map_err(|error| MusubiPinOutboxWireCaptureErrorV1::Check(error.into()))?;
            Ok(destination.0)
        })();
        match result {
            Ok(wire) => Ok(CapturedMusubiPinOutboxWireV1 {
                pending: self,
                wire,
            }),
            Err(error) => Err(MusubiPinOutboxWireCaptureFailureV1 {
                pending: self,
                error,
            }),
        }
    }
}

// Only the original admitted backing can grow its initialized prefix; append refuses growth
// beyond capacity and never allocates or substitutes an uncharged Vec.
pub(super) struct OriginalWireDestination(pub(super) ChargedBuffer<u8>);

impl io::Write for OriginalWireDestination {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.append(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
