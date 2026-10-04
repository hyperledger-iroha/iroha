//! Signature custody through the sole canonical SignedBlock field/frame walk.
//!
//! This prepared owner funds signature children and reusable decode controls only.
//! Other block/result children still use their canonical owning leaves and remain
//! explicit physical-custody obligations. No full-graph admission is claimed.

use super::{
    BlockPayload, BlockResult, BlockSignatureCustodyError, BlockSignatures, CommitCertificate,
    OutputFieldRef, PreparedBlockSignatures, SignedBlock, borrow_framed_signed_block_payload,
};
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_crypto::Hash;
use iroha_version::Version;
use norito::{
    SerializePayload,
    core::{
        CanonicalField, DecodeField, DecodeIntoError, FieldDestination, PreparedDecodeError,
        PreparedDecodeScopeError, PreparedDecodeWorkspace, PreparedRecordDestination, SequenceSpan,
    },
};

/// Original prepared scope, complete-frame, source or signature-custody failure.
#[derive(Debug, thiserror::Error)]
pub enum PreparedSignatureBlockError {
    /// Original pool/allocator refused the two actual canonical decode controls.
    #[error(transparent)]
    Storage(#[from] ChargedBufferError),
    /// Actual reusable canonical control construction/reuse failed.
    #[error(transparent)]
    Scope(#[from] PreparedDecodeScopeError),
    /// Original header/version failure before the complete canonical frame walk.
    #[error(transparent)]
    Frame(#[from] norito::core::DecodeAttemptError),
    /// Original canonical or signature destination failure, retaining every owner.
    #[error(transparent)]
    Decode(#[from] PreparedDecodeError<BlockSignatureCustodyError>),
    /// The original charged input, bytes or exact selected SignedBlockWire changed.
    #[error("prepared block signature decoder changed its original source")]
    SourceChanged,
    /// Source belongs to a different original pool or range is outside initialized input.
    #[error("prepared block signature decoder has no exact original funded input")]
    Source,
}
struct Source {
    address: usize,
    length: usize,
    hash: Hash,
    span: SequenceSpan,
}
impl Source {
    fn new(bytes: &[u8], span: SequenceSpan) -> Self {
        Self {
            address: bytes.as_ptr().addr(),
            length: bytes.len(),
            hash: Hash::new(bytes),
            span,
        }
    }
    fn matches(&self, bytes: &[u8], span: SequenceSpan) -> bool {
        self.address == bytes.as_ptr().addr()
            && self.length == bytes.len()
            && self.hash == Hash::new(bytes)
            && self.span == span
    }
}
/// Original-pool decoder controls and retained signature preparation for one source.
///
/// Construct before consuming a protocol attempt. The caller must retain the actual
/// input ChargedBuffer alive until success or explicit abandonment. Every refusal
/// preserves original signature preparation. Success shares those exact immutable
/// owners with SignedBlock while this decoder retains them through validation refusal.
/// Explicit retirement leaves the original block holding the same custody for publication/recovery.
/// TODO: prepare every other transaction, result, DA and authority graph child.
pub struct PreparedSignedBlockSignaturesDecode {
    workspace: PreparedDecodeWorkspace,
    pending: Option<PreparedBlockSignatures>,
    completed: Option<BlockSignatures>,
    source: Option<Source>,
    budget: AllocationBudget,
}
impl PreparedSignedBlockSignaturesDecode {
    /// Physically prepare both reusable canonical controls from one finite original pool.
    ///
    /// # Errors
    /// Returns exact original capacity/control/allocator refusal before source consumption.
    pub fn new(budget: &AllocationBudget) -> Result<Self, PreparedSignatureBlockError> {
        let mut reservation = budget
            .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
            .map_err(ChargedBufferError::Admission)?;
        let workspace = PreparedDecodeWorkspace::from_reservation(budget, &mut reservation)?;
        Ok(Self {
            workspace,
            pending: None,
            completed: None,
            source: None,
            budget: budget.clone(),
        })
    }
    /// Whether every retained control/collection/leaf keeps this same original pool.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.budget.same_pool(budget)
            && self.workspace.belongs_to(budget)
            && self
                .pending
                .as_ref()
                .is_none_or(|owner| owner.belongs_to(budget))
            && self
                .completed
                .as_ref()
                .is_none_or(|owner| owner.admitted_to(budget))
    }
    /// Abandon the exact source explicitly after its enclosing owner retires the attempt.
    /// Completed/partial signature custody is freed before it can fund another source.
    pub fn clear_consumed(&mut self) {
        self.pending = None;
        self.completed = None;
        self.source = None;
    }
    /// Borrow retained completed signature custody only for the unchanged original source.
    /// This proves allocation identity only; incomplete whole-frame verification grants no authority.
    ///
    /// # Errors
    /// Rejects a different pool, allocation or complete original source content.
    pub fn retained_signatures<'a>(
        &'a self,
        input: &ChargedBuffer<u8>,
    ) -> Result<Option<&'a BlockSignatures>, PreparedSignatureBlockError> {
        if !input.belongs_to(&self.budget) {
            return Err(PreparedSignatureBlockError::Source);
        }
        if let Some(source) = &self.source {
            if !source.matches(input.as_slice(), source.span) {
                return Err(PreparedSignatureBlockError::SourceChanged);
            }
        }
        Ok(self.completed.as_ref())
    }
    /// Decode the original complete SignedBlockWire through its one canonical field walk.
    ///
    /// # Errors
    /// Preserves partial signature owners and original canonical/error provenance.
    pub fn decode(
        &mut self,
        input: &ChargedBuffer<u8>,
        span: SequenceSpan,
        limits: norito::DecodeLimits,
    ) -> Result<SignedBlock, PreparedSignatureBlockError> {
        if !input.belongs_to(&self.budget) {
            return Err(PreparedSignatureBlockError::Source);
        }
        if let Some(source) = &self.source {
            if !source.matches(input.as_slice(), span) {
                return Err(PreparedSignatureBlockError::SourceChanged);
            }
        } else {
            self.source = Some(Source::new(input.as_slice(), span));
        }
        let bytes = span
            .get(input.as_slice())
            .map_err(|_| PreparedSignatureBlockError::Source)?;
        let framed = norito::core::classify_decode_attempt(|| {
            let (version, framed) = borrow_framed_signed_block_payload(bytes)?;
            if !SignedBlock::supported_versions().contains(&version) {
                return Err(norito::Error::UnsupportedVersion {
                    found: version,
                    expected: 1,
                });
            }
            Ok(framed)
        })?;
        let mut destination = Fields {
            input,
            budget: &self.budget,
            pending: &mut self.pending,
            completed: &mut self.completed,
            signature_ready: false,
            payload: None,
            result: None,
            certificate: None,
        };
        self.workspace
            .decode_canonical_into::<SignedBlock, _>(framed, limits, &mut destination)?;
        let block = SignedBlock {
            signatures: destination
                .completed
                .as_ref()
                .expect("complete original signature custody")
                .clone(),
            payload: destination
                .payload
                .take()
                .expect("complete canonical payload"),
            result: destination
                .result
                .take()
                .expect("complete canonical result field"),
            commit_certificate: destination
                .certificate
                .take()
                .expect("complete canonical certificate field"),
        };
        // Keep the same physical collection and original source throughout later validation
        // refusal. The enclosing owner explicitly retires this decoder after moving the
        // successful original into its retained execution/publication phase.
        Ok(block)
    }
}
struct Fields<'a> {
    input: &'a ChargedBuffer<u8>,
    budget: &'a AllocationBudget,
    pending: &'a mut Option<PreparedBlockSignatures>,
    completed: &'a mut Option<BlockSignatures>,
    signature_ready: bool,
    payload: Option<BlockPayload>,
    result: Option<Option<BlockResult>>,
    certificate: Option<Option<CommitCertificate>>,
}
impl FieldDestination for Fields<'_> {
    type Error = BlockSignatureCustodyError;
}
impl DecodeField<0, BlockSignatures> for Fields<'_> {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, BlockSignatures>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        field.with_payload(|bytes| {
            let start = bytes
                .as_ptr()
                .addr()
                .checked_sub(self.input.as_slice().as_ptr().addr())
                .ok_or(norito::Error::LengthMismatch)?;
            let end = start
                .checked_add(bytes.len())
                .ok_or(norito::Error::LengthMismatch)?;
            let plan = PreparedBlockSignatures::from_source(
                self.input,
                SequenceSpan { start, end },
                self.budget,
            )
            .map_err(|error| match error {
                BlockSignatureCustodyError::Decode(original) => {
                    DecodeIntoError::Codec(original.into_error())
                }
                other => DecodeIntoError::Destination(other),
            })?;
            if self.completed.is_none() {
                if self.pending.is_none() {
                    *self.pending = Some(plan);
                }
                self.pending
                    .as_mut()
                    .expect("same original pending signature owner")
                    .prepare(self.input)
                    .map_err(DecodeIntoError::Destination)?;
                match self
                    .pending
                    .take()
                    .expect("same prepared signatures")
                    .finish(self.input)
                {
                    Ok(owner) => *self.completed = Some(owner),
                    Err((owner, error)) => {
                        *self.pending = Some(owner);
                        return Err(DecodeIntoError::Destination(error));
                    }
                }
            }
            self.signature_ready = true;
            Ok(())
        })
    }
}
macro_rules! owned_field {
    ($index:literal,$ty:ty,$field:ident) => {
        impl DecodeField<$index, $ty> for Fields<'_> {
            type Value = ();
            fn decode_field(
                &mut self,
                field: CanonicalField<'_, $ty>,
            ) -> Result<(), DecodeIntoError<Self::Error>> {
                self.$field = Some(field.decode_owned()?);
                Ok(())
            }
        }
    };
}
owned_field!(1, BlockPayload, payload);
owned_field!(2, Option<BlockResult>, result);
owned_field!(3, Option<CommitCertificate>, certificate);
#[derive(norito::codec::Encode)]
struct Wire<'a> {
    signatures: OutputFieldRef<'a, BlockSignatures>,
    payload: OutputFieldRef<'a, BlockPayload>,
    result: OutputFieldRef<'a, Option<BlockResult>>,
    commit_certificate: OutputFieldRef<'a, Option<CommitCertificate>>,
}
impl SerializePayload for Fields<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        if !self.signature_ready {
            return Err(norito::Error::InvalidValue {
                context: "unfinished original signature fields",
            });
        }
        Wire {
            signatures: OutputFieldRef(
                self.completed
                    .as_ref()
                    .ok_or(norito::Error::LengthMismatch)?,
            ),
            payload: OutputFieldRef(self.payload.as_ref().ok_or(norito::Error::LengthMismatch)?),
            result: OutputFieldRef(self.result.as_ref().ok_or(norito::Error::LengthMismatch)?),
            commit_certificate: OutputFieldRef(
                self.certificate
                    .as_ref()
                    .ok_or(norito::Error::LengthMismatch)?,
            ),
        }
        .serialize(writer)
    }
}
impl PreparedRecordDestination<SignedBlock> for Fields<'_> {
    fn reset(&mut self) {
        self.signature_ready = false;
        self.payload = None;
        self.result = None;
        self.certificate = None;
    }
}

#[cfg(test)]
mod tests;
