//! UNLINKED DRAFT: bounded, inert exact-wire custody in a separate `pin-control-v1` directory.
//!
//! A slot consists of one canonical Norito claim and one opaque original SignedTransaction V1
//! wire. Exclusive creation claims the final names immediately; a writable, missing or partial
//! member keeps the slot occupied. Neither complete recovery nor an exact local retry means
//! "not submitted". This module has no Queue, deletion, replacement, terminalization, signer,
//! clock, Prepared/Pending/Verified Check decoder, or production owner/permit constructor.
//!
//! TODO: Before linking, the daemon must issue the private owner and slot permit only after
//! authenticating the original outbox marker/session through its current finalized State reader,
//! matching the reader's exact State allocation, checking an independently current checkpoint,
//! and admitting the COMPLETE control inventory's count and bytes. Both claim and wire count;
//! partial, unknown and interrupted files count too. No caller-supplied digest/count is authority.
//! The owner must retain the original process lock, directory lineage and their real funding.
//! TODO: Complete the canonical signed-wire producer with real original-pool backing and
//! retained live Check custody before constructing CapturedExactControlWireV1. The existing
//! encode_wire_v1 Vec is not funded by copying it into a ChargedBuffer after the fact.
//! TODO: Admit native filesystem/ACL/directory scratch and codec scratch through the original
//! owner, and qualify Windows; this Unix draft charges only its concrete Rust buffer backings.
//! The standalone exact-format census found decoder budget-context and alignment/padding
//! allocations beyond these buffers. Recovering with the current derived decoder is therefore
//! deliberately unqualified; preserving its original codec error does not fund that scratch.

use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_core::{execution_attempt::ExecutionDeferred, state::State};
use iroha_fs::PrivateDirectory;
use std::{io, sync::Arc};

use super::{
    MusubiPublicationPinOutboxHighWaterReadErrorV1, MusubiPublicationPinOutboxHighWaterReaderV1,
};

mod format;
mod storage;

use format::{ClaimBindingV1, ControlSlotV1, SlotNamesV1};

const CONTROL_DIRECTORY_V1: &str = "pin-control-v1";
const MAX_CLAIM_BYTES_V1: usize = 4 * 1024;
const MAX_CONTROL_RECORDS_V1: u32 = 1_024;
const MAX_CONTROL_BYTES_V1: u64 = 64 * 1024 * 1024;
const MAX_WIRE_BYTES_V1: usize =
    iroha_data_model::isi::musubi::MUSUBI_PIN_OUTBOX_EXTERNAL_MAX_BYTES_V1;

#[derive(Debug)]
enum ControlJournalErrorV1 {
    Deferred(ExecutionDeferred),
    LocallyAhead,
    Invalid,
    Conflict,
    OccupiedIncomplete,
    Capacity,
    Codec(norito::Error),
    Storage(io::Error),
}

impl From<ChargedBufferError> for ControlJournalErrorV1 {
    fn from(error: ChargedBufferError) -> Self {
        match error {
            ChargedBufferError::Admission(original) => Self::Deferred(original.into()),
            ChargedBufferError::Allocator { .. } => {
                Self::Deferred(ivm::error::ExecutionDeferral::AllocationUnavailable.into())
            }
        }
    }
}

impl From<MusubiPublicationPinOutboxHighWaterReadErrorV1> for ControlJournalErrorV1 {
    fn from(error: MusubiPublicationPinOutboxHighWaterReadErrorV1) -> Self {
        match error {
            MusubiPublicationPinOutboxHighWaterReadErrorV1::Deferred(original) => {
                Self::Deferred(original)
            }
            MusubiPublicationPinOutboxHighWaterReadErrorV1::LocallyAhead => Self::LocallyAhead,
            MusubiPublicationPinOutboxHighWaterReadErrorV1::Invalid => Self::Invalid,
        }
    }
}

/// No production constructor: a resource-pool handle or matching marker bytes is insufficient.
struct OriginalControlOwnerV1<'a> {
    directory: &'a PrivateDirectory,
    state: &'a Arc<State>,
    reader: &'a MusubiPublicationPinOutboxHighWaterReaderV1,
    budget: AllocationBudget,
    binding: ClaimBindingV1,
    limits: ControlJournalLimitsV1,
}

/// Configuration snapshot; validating these numbers does not admit an inventory or a slot.
#[derive(Clone, Copy)]
struct ControlJournalLimitsV1 {
    max_records: u32,
    max_total_bytes: u64,
}

impl ControlJournalLimitsV1 {
    fn validate(self) -> Result<(), ControlJournalErrorV1> {
        if self.max_records == 0
            || self.max_records > MAX_CONTROL_RECORDS_V1
            || self.max_total_bytes == 0
            || self.max_total_bytes > MAX_CONTROL_BYTES_V1
        {
            return Err(ControlJournalErrorV1::Invalid);
        }
        Ok(())
    }
}

/// No production constructor: complete original inventory admission must precede file mutation.
/// This fixed extent is consumed by the original slot, including after partial native failure.
struct AdmittedControlSlotV1<'a> {
    owner: &'a OriginalControlOwnerV1<'a>,
    slot: ControlSlotV1,
    names: SlotNamesV1,
    admitted_record_bytes: u64,
}

/// No production constructor: a future exact-wire signer owner transfers these original bytes.
/// C may be a live PendingMusubiPinOutboxCheckV1; no operation below clones or rebuilds it.
struct CapturedExactControlWireV1<C> {
    custody: C,
    bytes: ChargedBuffer<u8>,
    binding: ClaimBindingV1,
    slot: ControlSlotV1,
}

/// Even a successful local retry returns custody, not a permission to submit the transaction.
struct StoredControlCustodyV1<C> {
    original: CapturedExactControlWireV1<C>,
}

/// Every refusal retains the same live custody and the exact original funded wire.
struct ControlStoreFailureV1<C> {
    original: CapturedExactControlWireV1<C>,
    error: ControlJournalErrorV1,
}

/// Restart recovers inert bytes only. There is deliberately no conversion into live Check stages.
struct RecoveredControlBytesV1<'a> {
    state: &'a Arc<State>,
    bytes: ChargedBuffer<u8>,
}

impl RecoveredControlBytesV1<'_> {
    fn exact_wire(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

#[cfg(test)]
mod tests;
