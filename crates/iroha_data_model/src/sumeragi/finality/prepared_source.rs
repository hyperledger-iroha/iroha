//! Prepared canonical journal ranges into the caller's original immutable frame.
//!
//! The generated artifact/journal field walk remains the only wire traversal. These
//! destinations borrow raw byte leaves and retain physically funded spans instead of
//! copying each artifact body. Indexed views retain a borrow of the same original
//! charged source; ordinary owned journals cannot create this provenance.
//! TODO: physically fund every decoded `SignedBlock` payload/result/DA and authority
//! graph. This storage seam grants no finality or whole-native-graph guarantee.

use super::{NativeFinalityArtifact, NativeFinalityJournal, NativeFinalityLimits};
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_crypto::Hash;
use norito::SerializePayload;
use norito::core::{
    CanonicalField, DecodeField, DecodeFromSlice, DecodeIntoError, DecodeRecordFields, Encoder,
    FieldDestination, PayloadRef, PreparedDecodeError, PreparedDecodeScopeError,
    PreparedDecodeWorkspace, PreparedRecordDestination, SequenceDestinationError, SequenceSpan,
    prepare_element_sequence,
};

#[derive(Clone, Copy)]
enum Frames<'a> {
    Owned(&'a [NativeFinalityArtifact]),
    Indexed {
        source: &'a ChargedBuffer<u8>,
        spans: &'a [SequenceSpan],
    },
}

/// One borrowed native journal source, with one canonical `SignedBlockWire` per height.
/// This transport view grants no finality authority and never decodes blocks itself.
/// The consuming block/native verifier must validate each opaque wire and protocol.
#[derive(Clone, Copy)]
pub struct NativeFinalitySource<'a> {
    frames: Frames<'a>,
}
impl<'a> From<&'a NativeFinalityJournal> for NativeFinalitySource<'a> {
    fn from(journal: &'a NativeFinalityJournal) -> Self {
        Self {
            frames: Frames::Owned(&journal.blocks),
        }
    }
}
impl<'a> NativeFinalitySource<'a> {
    /// Exact number of original consecutive source frames.
    #[must_use]
    pub fn len(&self) -> usize {
        match self.frames {
            Frames::Owned(frames) => frames.len(),
            Frames::Indexed { spans, .. } => spans.len(),
        }
    }
    /// Whether the source has no signed genesis frame.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Borrow each original canonical wire without copying its bytes.
    #[must_use]
    pub fn frames(self) -> NativeFinalityFrames<'a> {
        NativeFinalityFrames {
            source: self,
            index: 0,
        }
    }
    /// Check the same exact source count, per-block and aggregate bounds for every custody form.
    ///
    /// # Errors
    /// Rejects invalid limits, empty or oversized sources and aggregate overflow.
    pub fn validate(self, limits: NativeFinalityLimits) -> Result<(), String> {
        limits.validate()?;
        if self.is_empty() || self.len() > limits.block_count {
            return Err("native journal block count exceeds its configured bound".into());
        }
        let mut total = 0_usize;
        for frame in self.frames() {
            let wire = frame.wire();
            if wire.is_empty() || wire.len() > limits.block_bytes {
                return Err("native journal contains an oversized or empty frame".into());
            }
            total = total
                .checked_add(wire.len())
                .ok_or("native journal size overflow")?;
            if total > limits.journal_bytes {
                return Err("native journal exceeds its configured aggregate byte bound".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Frame<'a> {
    Owned(&'a [u8]),
    Charged(NativeFinalityChargedFrame<'a>),
}

/// One original native frame with privately constructed storage provenance.
///
/// A legitimate ordinary journal exposes its original wire without charged
/// provenance. Only the complete prepared canonical journal can create a charged
/// frame. Neither form authenticates a block, quorum or native finality.
#[derive(Clone, Copy)]
pub struct NativeFinalityFrame<'a> {
    frame: Frame<'a>,
}
impl<'a> NativeFinalityFrame<'a> {
    /// Borrow the exact original block wire bytes without copying them.
    #[must_use]
    pub fn wire(&self) -> &'a [u8] {
        match self.frame {
            Frame::Owned(wire) => wire,
            Frame::Charged(source) => source.wire(),
        }
    }

    /// Borrow charged provenance only when the prepared canonical source established it.
    #[must_use]
    pub fn charged_source(&self) -> Option<NativeFinalityChargedFrame<'a>> {
        match self.frame {
            Frame::Owned(_) => None,
            Frame::Charged(source) => Some(source),
        }
    }
}

/// A canonically bounded frame borrowing its original charged journal allocation.
///
/// Fields and construction are private. The borrow keeps the source backing and
/// charge inseparable while it is in use. This proves original storage identity
/// only; the consuming native verifier still authenticates the complete protocol.
#[derive(Clone, Copy)]
pub struct NativeFinalityChargedFrame<'a> {
    source: &'a ChargedBuffer<u8>,
    span: SequenceSpan,
}
impl<'a> NativeFinalityChargedFrame<'a> {
    /// Borrow the original complete source owner, never a projected or copied wire.
    #[must_use]
    pub fn original_source(&self) -> &'a ChargedBuffer<u8> {
        self.source
    }

    /// Exact canonical wire range within the original initialized source backing.
    #[must_use]
    pub fn span(&self) -> SequenceSpan {
        self.span
    }

    /// Borrow the canonically validated range from the same original source.
    #[must_use]
    pub fn wire(&self) -> &'a [u8] {
        &self.source.as_slice()[self.span.start..self.span.end]
    }

    /// Whether this actual original source owner belongs to the supplied finite pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.source.belongs_to(budget)
    }
}

/// Exact borrowed sequence of original native frames from one immutable source.
pub struct NativeFinalityFrames<'a> {
    source: NativeFinalitySource<'a>,
    index: usize,
}
impl<'a> Iterator for NativeFinalityFrames<'a> {
    type Item = NativeFinalityFrame<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let frame = match self.source.frames {
            Frames::Owned(frames) => Frame::Owned(frames.get(self.index)?.block_wire.as_slice()),
            Frames::Indexed { source, spans } => {
                let span = *spans.get(self.index)?;
                // Only the complete private canonical destination supplies these
                // original-source ranges; keep the existing checked range boundary.
                source.as_slice().get(span.start..span.end)?;
                Frame::Charged(NativeFinalityChargedFrame { source, span })
            }
        };
        self.index += 1;
        Some(NativeFinalityFrame { frame })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.source.len() - self.index;
        (left, Some(left))
    }
}
impl ExactSizeIterator for NativeFinalityFrames<'_> {}

/// A retained prepared source refusal, separate from malformed canonical bytes.
#[derive(Debug, thiserror::Error)]
pub enum PreparedNativeFinalityDestinationError {
    /// The unchanged prepared sequence span owner refused the input geometry.
    #[error(transparent)]
    Sequence(#[from] SequenceDestinationError),
    /// A derived field no longer points into the same original source frame.
    #[error("native journal field does not belong to its original source")]
    Source,
}
/// Original preparation, canonical decoding or immutable-source custody failure.
#[derive(Debug, thiserror::Error)]
pub enum PreparedNativeFinalityError {
    /// The actual source allocation belongs to another original finite pool.
    #[error("native journal source belongs to another original pool")]
    ForeignPool,
    /// Explicit source bounds were invalid or a decoded source exceeded them.
    #[error("native prepared journal: {0}")]
    Invalid(String),
    /// Original pool or physical allocator refused prepared span backing.
    #[error(transparent)]
    Storage(#[from] ChargedBufferError),
    /// Original prepared canonical decoder control construction or reuse failed.
    #[error(transparent)]
    Scope(#[from] PreparedDecodeScopeError),
    /// Original canonical or destination refusal; all prepared owners remain live.
    #[error(transparent)]
    Decode(#[from] PreparedDecodeError<PreparedNativeFinalityDestinationError>),
    /// Retry offered a different allocation or modified bytes for the retained source.
    #[error("native journal retry changed its original source")]
    SourceChanged,
    /// The original complete canonical source has not yet been decoded.
    #[error("native journal has no complete prepared source")]
    NotDecoded,
}
impl From<String> for PreparedNativeFinalityError {
    fn from(error: String) -> Self {
        Self::Invalid(error)
    }
}
type DestinationResult<T> = Result<T, DecodeIntoError<PreparedNativeFinalityDestinationError>>;

struct SourceIdentity {
    address: usize,
    length: usize,
    hash: Hash,
}
impl SourceIdentity {
    fn new(bytes: &[u8]) -> Self {
        Self {
            address: bytes.as_ptr().addr(),
            length: bytes.len(),
            hash: Hash::new(bytes),
        }
    }
    fn matches(&self, bytes: &[u8]) -> bool {
        self.address == bytes.as_ptr().addr()
            && self.length == bytes.len()
            && self.hash == Hash::new(bytes)
    }
}

/// Original-pool canonical field controls and fixed journal range backing.
///
/// Construct this owner before the private protocol attempt begins. Every decode
/// borrows its complete original input; no artifact byte Vec, late span allocation,
/// owning-decoder fallback or alternate wire representation is created. Retry keeps
/// that source's allocation identity as well as its complete byte hash. The caller
/// retains its actual charged input until success or explicit abandonment; returned
/// views borrow that same owner and cannot outlive or mutate its allocation.
pub struct PreparedNativeFinalityJournal {
    spans: ChargedBuffer<SequenceSpan>,
    blocks: ChargedBuffer<SequenceSpan>,
    workspace: PreparedDecodeWorkspace,
    budget: AllocationBudget,
    limits: NativeFinalityLimits,
    source: Option<SourceIdentity>,
    count: Option<usize>,
}
impl PreparedNativeFinalityJournal {
    /// Prepare actual initialized range backing and canonical decode controls.
    ///
    /// # Errors
    /// Returns original pool/physical/control refusal before source consumption.
    pub fn new(
        limits: NativeFinalityLimits,
        budget: &AllocationBudget,
    ) -> Result<Self, PreparedNativeFinalityError> {
        limits.validate()?;
        let mut spans = ChargedBuffer::new(limits.block_count, budget)?;
        let mut blocks = ChargedBuffer::new(limits.block_count, budget)?;
        for _ in 0..limits.block_count {
            spans.push_reserved(SequenceSpan { start: 0, end: 0 });
            blocks.push_reserved(SequenceSpan { start: 0, end: 0 });
        }
        let mut reservation = budget
            .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
            .map_err(ChargedBufferError::Admission)?;
        let workspace = PreparedDecodeWorkspace::from_reservation(budget, &mut reservation)?;
        Ok(Self {
            spans,
            blocks,
            workspace,
            budget: budget.clone(),
            limits,
            source: None,
            count: None,
        })
    }
    /// Whether this owner retains the exact original operation pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.budget.same_pool(budget) && self.workspace.belongs_to(budget)
    }
    /// Whether a complete canonical source is already retained.
    #[must_use]
    pub fn is_decoded(&self) -> bool {
        self.count.is_some()
    }
    /// Retire a semantically consumed input while keeping all prepaid backing.
    /// Call only after the enclosing attempt has consumed that exact frame.
    pub fn clear_consumed(&mut self) {
        self.count = None;
        self.source = None;
    }
    /// Decode at most once, retaining exact source and initialized backing on refusal.
    ///
    /// # Errors
    /// Rejects a foreign source pool before pinning or canonical work. Preserves
    /// original canonical/refusal classification and changed-source custody.
    pub fn decode(&mut self, input: &ChargedBuffer<u8>) -> Result<(), PreparedNativeFinalityError> {
        if !input.belongs_to(&self.budget) {
            return Err(PreparedNativeFinalityError::ForeignPool);
        }
        let bytes = input.as_slice();
        if bytes.is_empty() || bytes.len() > self.limits.journal_bytes {
            return Err(PreparedNativeFinalityError::Invalid(
                "native journal archive exceeds its configured byte bound".into(),
            ));
        }
        if let Some(source) = &self.source {
            if !source.matches(bytes) {
                return Err(PreparedNativeFinalityError::SourceChanged);
            }
        } else {
            self.source = Some(SourceIdentity::new(bytes));
        }
        if self.count.is_none() {
            let mut destination = JournalDestination {
                source: bytes,
                spans: &mut self.spans,
                blocks: &mut self.blocks,
                count: None,
                limit: self.limits.block_count,
            };
            self.workspace
                .decode_canonical_into::<NativeFinalityJournal, _>(
                    bytes,
                    self.limits.decode_limits()?,
                    &mut destination,
                )?;
            self.count = destination.count;
            // Keep the same final source validation used by the owning canonical decoder.
            if let Err(error) = self.view(input)?.validate(self.limits) {
                self.count = None;
                return Err(error.into());
            }
        }
        Ok(())
    }
    /// Borrow only the unchanged, completely canonical original source.
    ///
    /// # Errors
    /// Rejects a foreign pool before other checks, a different allocation/content,
    /// or an unfinished canonical decode.
    pub fn view<'a>(
        &'a self,
        input: &'a ChargedBuffer<u8>,
    ) -> Result<NativeFinalitySource<'a>, PreparedNativeFinalityError> {
        if !input.belongs_to(&self.budget) {
            return Err(PreparedNativeFinalityError::ForeignPool);
        }
        let bytes = input.as_slice();
        let count = self.count.ok_or(PreparedNativeFinalityError::NotDecoded)?;
        if !self
            .source
            .as_ref()
            .is_some_and(|source| source.matches(bytes))
        {
            return Err(PreparedNativeFinalityError::SourceChanged);
        }
        Ok(NativeFinalitySource {
            frames: Frames::Indexed {
                source: input,
                spans: &self.blocks.as_slice()[..count],
            },
        })
    }
}

struct ArtifactDestination<'a> {
    source: &'a [u8],
    span: Option<SequenceSpan>,
}
impl FieldDestination for ArtifactDestination<'_> {
    type Error = PreparedNativeFinalityDestinationError;
}
impl DecodeField<0, Vec<u8>> for ArtifactDestination<'_> {
    type Value = ();
    fn decode_field(&mut self, field: CanonicalField<'_, Vec<u8>>) -> DestinationResult<()> {
        field.with_payload(|bytes| {
            let (wire, used) = <&[u8] as DecodeFromSlice>::decode_from_slice(bytes)?;
            // Match owning Vec<u8>'s nominal logical byte charge and its
            // precedence over the enclosing field-consumption check. Actual
            // bytes stay in the funded input frame; counters are not custody.
            norito::core::reserve_decode_allocation(wire.len())?;
            if used != bytes.len() {
                return Err(norito::Error::LengthMismatch.into());
            }
            let start = wire
                .as_ptr()
                .addr()
                .checked_sub(self.source.as_ptr().addr())
                .ok_or(DecodeIntoError::Destination(
                    PreparedNativeFinalityDestinationError::Source,
                ))?;
            let end = start
                .checked_add(wire.len())
                .ok_or(DecodeIntoError::Destination(
                    PreparedNativeFinalityDestinationError::Source,
                ))?;
            if self
                .source
                .get(start..end)
                .is_none_or(|original| original.as_ptr() != wire.as_ptr())
            {
                return Err(DecodeIntoError::Destination(
                    PreparedNativeFinalityDestinationError::Source,
                ));
            }
            self.span = Some(SequenceSpan { start, end });
            Ok(())
        })
    }
}
struct JournalDestination<'a, 'storage> {
    source: &'a [u8],
    spans: &'storage mut ChargedBuffer<SequenceSpan>,
    blocks: &'storage mut ChargedBuffer<SequenceSpan>,
    count: Option<usize>,
    limit: usize,
}
impl FieldDestination for JournalDestination<'_, '_> {
    type Error = PreparedNativeFinalityDestinationError;
}
impl DecodeField<0, Vec<NativeFinalityArtifact>> for JournalDestination<'_, '_> {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, Vec<NativeFinalityArtifact>>,
    ) -> DestinationResult<()> {
        field.with_payload(|bytes| {
            let plan =
                prepare_element_sequence(bytes, self.spans.as_mut_slice()).map_err(|error| {
                    match error {
                        SequenceDestinationError::Codec(error) => DecodeIntoError::Codec(error),
                        // The shared walker first admits logical count/work and
                        // checks original framing. A declared count beyond the
                        // independently pinned protocol bound is invalid input,
                        // rather than a request to enlarge the prepared bank.
                        SequenceDestinationError::Storage { required, .. }
                            if required > self.limit =>
                        {
                            DecodeIntoError::Codec(norito::Error::InvalidValue {
                                context: "native journal block count exceeds its configured bound",
                            })
                        }
                        error => DecodeIntoError::Destination(
                            PreparedNativeFinalityDestinationError::Sequence(error),
                        ),
                    }
                })?;
            if plan.used() != bytes.len() {
                return Err(norito::Error::LengthMismatch.into());
            }
            let blocks = &mut *self.blocks;
            let source = self.source;
            plan.decode_elements::<NativeFinalityArtifact, PreparedNativeFinalityDestinationError>(
                |index, field| {
                    let mut artifact = ArtifactDestination { source, span: None };
                    field.with_payload(|bytes| {
                        let (_, used) =
                            NativeFinalityArtifact::decode_fields(bytes, &mut artifact)?;
                        if used != bytes.len() {
                            return Err(norito::Error::LengthMismatch.into());
                        }
                        Ok(())
                    })?;
                    blocks.as_mut_slice()[index] =
                        artifact.span.ok_or(DecodeIntoError::Destination(
                            PreparedNativeFinalityDestinationError::Source,
                        ))?;
                    Ok(())
                },
            )?;
            self.count = Some(plan.len());
            Ok(())
        })
    }
}
impl PreparedRecordDestination<NativeFinalityJournal> for JournalDestination<'_, '_> {
    fn reset(&mut self) {
        self.count = None;
    }
}
#[derive(norito::NoritoSerialize)]
struct ArtifactView<'a> {
    block_wire: &'a [u8],
}
struct ArtifactViews<'a> {
    source: &'a [u8],
    spans: &'a [SequenceSpan],
}
impl SerializePayload for ArtifactViews<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::write_element_sequence::<ArtifactView<'_>, _>(
            writer,
            self.spans.iter().map(|span| ArtifactView {
                block_wire: &self.source[span.start..span.end],
            }),
        )
    }
}
impl SerializePayload for JournalDestination<'_, '_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::Error> {
        #[derive(norito::NoritoSerialize)]
        struct View<'a> {
            blocks: PayloadRef<'a, ArtifactViews<'a>>,
        }
        let count = self.count.ok_or(norito::Error::InvalidValue {
            context: "unfinished native journal destination",
        })?;
        let blocks = ArtifactViews {
            source: self.source,
            spans: &self.blocks.as_slice()[..count],
        };
        View {
            blocks: PayloadRef(&blocks),
        }
        .serialize(writer)
    }
}

#[cfg(test)]
mod tests;
