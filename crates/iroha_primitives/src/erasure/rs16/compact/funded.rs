//! Exact original-pool backing for compact codec jobs; no protocol admission or scheduling.
//! Inputs and the shared RS16 field tables have separate existing owners.
use super::CompactShape;
use crate::erasure::rs16::Rs16Error;
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer, PrepaidBufferError};
use std::alloc::Layout;
/// A resource refusal or an invalid RS16 input encountered by a funded codec job.
#[derive(Debug)]
pub enum CodecAllocationError {
    /// The original pool cannot reserve the exact job backing.
    Admission(AllocationRefusal),
    /// Allocation of already reserved backing failed.
    Allocation(PrepaidBufferError),
    /// The input geometry, shards, or reconstructed codeword is invalid.
    Codec(Rs16Error),
}
impl std::fmt::Display for CodecAllocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(f),
            Self::Allocation(error) => error.fmt(f),
            Self::Codec(_) => f.write_str("invalid compact RS16 codeword"),
        }
    }
}
impl std::error::Error for CodecAllocationError {}
/// Move-only encoded bytes retaining their original pool charge.
pub struct Encoded {
    shape: CompactShape,
    codeword: ChargedBuffer<u8>,
}
impl Encoded {
    /// Exact checked geometry of this job.
    pub fn shape(&self) -> CompactShape {
        self.shape
    }
    /// Original encoded bytes in canonical stripe and row order.
    pub fn codeword(&self) -> &[u8] {
        self.codeword.as_slice()
    }
    /// Whether every retained output is charged to this exact pool.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.codeword.belongs_to(budget)
    }
}
/// Verified reconstructed job outputs; no safe backing extraction or growth.
pub struct Reconstructed {
    shape: CompactShape,
    payload: ChargedBuffer<u8>,
    codeword: ChargedBuffer<u8>,
}
impl Reconstructed {
    /// Move verified payload backing unchanged, dropping the no-longer-needed codeword.
    pub fn into_payload(self) -> ChargedBuffer<u8> {
        self.payload
    }
    /// Exact checked geometry of this job.
    pub fn shape(&self) -> CompactShape {
        self.shape
    }
    /// Reconstructed original payload without padding.
    pub fn payload(&self) -> &[u8] {
        self.payload.as_slice()
    }
    /// Original encoded bytes in canonical stripe and row order.
    pub fn codeword(&self) -> &[u8] {
        self.codeword.as_slice()
    }
    /// Whether every retained output is charged to this exact pool.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.payload.belongs_to(budget) && self.codeword.belongs_to(budget)
    }
}
fn layout<T>(count: usize) -> Result<Layout, CodecAllocationError> {
    Layout::array::<T>(count)
        .map_err(|_| CodecAllocationError::Admission(AllocationRefusal::DemandOverflow))
}
/// Total exact requested backing bytes, excluding the input and existing process tables.
///
/// # Errors
/// Returns [`CodecAllocationError::Admission`] if a buffer layout or the sum of
/// all required layouts exceeds the host's address space.
pub fn required_backing_bytes(
    shape: CompactShape,
    reconstruct: bool,
) -> Result<usize, CodecAllocationError> {
    [
        layout::<u8>(shape.encoded_bytes())?,
        layout::<u8>(if reconstruct {
            shape.payload_bytes()
        } else {
            0
        })?,
        layout::<u16>(shape.workspace_words())?,
    ]
    .into_iter()
    .try_fold(0usize, |n, v| {
        n.checked_add(v.size())
            .ok_or(CodecAllocationError::Admission(
                AllocationRefusal::DemandOverflow,
            ))
    })
}
type JobBuffers = (ChargedBuffer<u8>, ChargedBuffer<u8>, ChargedBuffer<u16>);

fn buffers(
    shape: CompactShape,
    reconstruct: bool,
    budget: &AllocationBudget,
) -> Result<JobBuffers, CodecAllocationError> {
    let mut reservation = budget
        .try_reserve_bytes(required_backing_bytes(shape, reconstruct)?)
        .map_err(CodecAllocationError::Admission)?;
    let mut codeword = ChargedBuffer::from_reservation(shape.encoded_bytes(), &mut reservation)
        .map_err(CodecAllocationError::Allocation)?;
    let mut payload = ChargedBuffer::from_reservation(
        if reconstruct {
            shape.payload_bytes()
        } else {
            0
        },
        &mut reservation,
    )
    .map_err(CodecAllocationError::Allocation)?;
    let mut workspace = ChargedBuffer::from_reservation(shape.workspace_words(), &mut reservation)
        .map_err(CodecAllocationError::Allocation)?;
    for _ in 0..codeword.capacity() {
        codeword.push_reserved(0);
    }
    for _ in 0..payload.capacity() {
        payload.push_reserved(0);
    }
    for _ in 0..workspace.capacity() {
        workspace.push_reserved(0);
    }
    Ok((codeword, payload, workspace))
}
/// Encode into one original-funded move-only byte owner; scratch drops before return.
///
/// # Errors
/// Returns a resource error when the exact backing cannot be reserved or allocated,
/// or [`CodecAllocationError::Codec`] when the payload does not fit the shape or
/// the encoding matrix cannot be inverted.
pub fn encode_funded(
    shape: CompactShape,
    payload: &[u8],
    budget: &AllocationBudget,
) -> Result<Encoded, CodecAllocationError> {
    let (mut codeword, _, mut workspace) = buffers(shape, false, budget)?;
    shape
        .encode_into(payload, codeword.as_mut_slice(), workspace.as_mut_slice())
        .map_err(CodecAllocationError::Codec)?;
    Ok(Encoded { shape, codeword })
}
/// Recover, compare all original chunk commitments and canonical padding before exposing owners.
///
/// # Errors
/// Returns a resource error when the exact backing cannot be reserved or allocated,
/// or [`CodecAllocationError::Codec`] when rows are missing, malformed, inconsistent,
/// rejected by the commitment predicate, or reconstruct noncanonical padding.
pub fn reconstruct_funded(
    shape: CompactShape,
    received: &[Option<&[u8]>],
    budget: &AllocationBudget,
    verify_original_row: impl FnMut(usize, &[u8]) -> bool,
) -> Result<Reconstructed, CodecAllocationError> {
    let (mut codeword, mut payload, mut workspace) = buffers(shape, true, budget)?;
    shape
        .reconstruct_into(
            received,
            payload.as_mut_slice(),
            codeword.as_mut_slice(),
            workspace.as_mut_slice(),
            verify_original_row,
        )
        .map_err(CodecAllocationError::Codec)?;
    Ok(Reconstructed {
        shape,
        payload,
        codeword,
    })
}
