//! Prepared canonical certificate leaves and their unchanged original shared control.
#![doc = include_str!("prepared.md")]
//!
//! The enclosing canonical field/frame owner supplies the advertised flags and
//! verifies complete framing. This owner funds only the four opaque certificate
//! byte leaves and their immutable control, never their decoded semantic graphs.

use super::{CanonicalParts, ChargedCertificateParts, CommitCertificate, Storage};
use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, ChargedBuffer,
    ChargedBufferFromChargeError, ChargedShared, ReservedChargedShared, SharedFromChargeError,
};
use iroha_crypto::Hash;
use norito::core::{
    CanonicalField, DecodeField, DecodeFromSlice, DecodeIntoError, DecodeRecordFields,
    FieldDestination, SequenceDestinationError, SequenceSpan,
};
use std::alloc::Layout;

/// Original certificate source, canonical cause or exact physical custody refusal.
#[derive(Debug, thiserror::Error)]
pub enum CertificateCustodyError {
    /// Input or a retained backing belongs to another original finite pool.
    #[error("certificate source belongs to another original pool")]
    ForeignPool,
    /// Complete input address, length, content or selected field changed.
    #[error("certificate retry changed its complete original source")]
    SourceChanged,
    /// A selected field is outside initialized original input.
    #[error("certificate field is outside its original source")]
    SourceRange,
    /// The enclosing canonical field has not installed its advertised layout.
    #[error("certificate preparation requires its original advertised layout")]
    MissingLayout,
    /// An unchanged source was retried under a different advertised layout.
    #[error("certificate retry changed its original advertised layout")]
    LayoutChanged,
    /// Exact canonical byte or enclosing logical-limit cause.
    #[error(transparent)]
    Decode(#[from] norito::core::DecodeAttemptError),
    /// Original pool refused all actual leaf/control layouts before allocation.
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    /// A physically prepaid original leaf allocation was refused.
    #[error(transparent)]
    Buffer(#[from] ChargedBufferFromChargeError),
    /// The physically prepaid immutable shared control was refused.
    #[error(transparent)]
    Control(#[from] SharedFromChargeError),
    /// No complete canonical field fill has succeeded for this original source.
    #[error("certificate preparation is incomplete")]
    Incomplete,
}
struct Identity {
    address: usize,
    length: usize,
    hash: Hash,
}
impl Identity {
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
struct Plan<'a> {
    source: &'a [u8],
    leaves: [Option<SequenceSpan>; 4],
}
impl FieldDestination for Plan<'_> {
    type Error = std::convert::Infallible;
}
impl<const INDEX: usize> DecodeField<INDEX, Vec<u8>> for Plan<'_> {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, Vec<u8>>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        field.with_payload(|bytes| {
            let (leaf, used) = <&[u8] as DecodeFromSlice>::decode_from_slice(bytes)?;
            if used != bytes.len() {
                return Err(norito::Error::LengthMismatch.into());
            }
            let start = leaf
                .as_ptr()
                .addr()
                .checked_sub(self.source.as_ptr().addr())
                .ok_or(norito::Error::LengthMismatch)?;
            let end = start
                .checked_add(leaf.len())
                .ok_or(norito::Error::LengthMismatch)?;
            let span = SequenceSpan { start, end };
            if span.get(self.source)? != leaf {
                return Err(norito::Error::LengthMismatch.into());
            }
            *self
                .leaves
                .get_mut(INDEX)
                .ok_or(norito::Error::LengthMismatch)? = Some(span);
            Ok(())
        })
    }
}
struct Fill<'a> {
    values: &'a mut [Option<ChargedBuffer<u8>>; 4],
}
impl FieldDestination for Fill<'_> {
    type Error = std::convert::Infallible;
}
impl<const INDEX: usize> DecodeField<INDEX, Vec<u8>> for Fill<'_> {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, Vec<u8>>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        field.with_payload(|bytes| {
            let value = self
                .values
                .get_mut(INDEX)
                .and_then(Option::as_mut)
                .ok_or(norito::Error::LengthMismatch)?;
            let (length, used) =
                norito::core::decode_raw_byte_sequence_into(bytes, value.as_mut_slice()).map_err(
                    |error| match error {
                        SequenceDestinationError::Codec(original) => original,
                        SequenceDestinationError::Storage { .. } => norito::Error::LengthMismatch,
                    },
                )?;
            if used != bytes.len() || length != value.as_slice().len() {
                return Err(norito::Error::LengthMismatch.into());
            }
            Ok(())
        })
    }
}

/// Four exact original certificate backing allocations and a prepaid shared shell.
///
/// Construct inside the canonical enclosing field's advertised layout. Planning
/// borrows the sole generated four-field traversal. All five actual layouts are
/// admitted together before any destination allocation or byte fill. An allocator
/// refusal keeps every original charge and completed sibling allocation; retry
/// never acquires replacement credit or invokes an ordinary heap decoder.
/// TODO: compose the separate decoded header/QC/result/availability semantic graphs.
pub struct PreparedCommitCertificate {
    source: Identity,
    span: SequenceSpan,
    leaves: [SequenceSpan; 4],
    flags: u8,
    values: [Option<ChargedBuffer<u8>>; 4],
    charges: [Option<AllocationCharge>; 5],
    shell: Option<ReservedChargedShared<ChargedCertificateParts>>,
    admitted: bool,
    ready: bool,
    budget: AllocationBudget,
}
impl PreparedCommitCertificate {
    /// Measure the four canonical byte leaves in the actual charged original source.
    ///
    /// # Errors
    /// Preserves canonical/depth/field/count causes and rejects changed pool/range/layout.
    pub fn from_source(
        input: &ChargedBuffer<u8>,
        span: SequenceSpan,
        budget: &AllocationBudget,
    ) -> Result<Self, CertificateCustodyError> {
        if !input.belongs_to(budget) {
            return Err(CertificateCustodyError::ForeignPool);
        }
        let bytes = span
            .get(input.as_slice())
            .map_err(|_| CertificateCustodyError::SourceRange)?;
        let flags =
            norito::core::effective_decode_flags().ok_or(CertificateCustodyError::MissingLayout)?;
        let leaves = norito::core::classify_decode_attempt(|| {
            let mut plan = Plan {
                source: input.as_slice(),
                leaves: [None; 4],
            };
            let (_, used) = CanonicalParts::decode_fields(bytes, &mut plan)
                .map_err(DecodeIntoError::into_codec)?;
            if used != bytes.len() {
                return Err(norito::Error::LengthMismatch);
            }
            let mut leaves = [SequenceSpan { start: 0, end: 0 }; 4];
            for (target, source) in leaves.iter_mut().zip(plan.leaves) {
                *target = source.ok_or(norito::Error::LengthMismatch)?;
            }
            Ok(leaves)
        })?;
        Ok(Self {
            source: Identity::new(input.as_slice()),
            span,
            leaves,
            flags,
            values: std::array::from_fn(|_| None),
            charges: std::array::from_fn(|_| None),
            shell: None,
            admitted: false,
            ready: false,
            budget: budget.clone(),
        })
    }
    fn check(&self, input: &ChargedBuffer<u8>) -> Result<(), CertificateCustodyError> {
        if !input.belongs_to(&self.budget) {
            return Err(CertificateCustodyError::ForeignPool);
        }
        if !self.source.matches(input.as_slice()) {
            return Err(CertificateCustodyError::SourceChanged);
        }
        Ok(())
    }
    /// Exact leaf and shared-control layouts, before any allocator call.
    ///
    /// # Errors
    /// Rejects an unrepresentable layout; these counts never grant protocol authority.
    pub fn allocation_layouts(&self) -> Result<[Layout; 5], AllocationRefusal> {
        let mut layouts = [Layout::new::<u8>(); 5];
        for (layout, span) in layouts[..4].iter_mut().zip(self.leaves) {
            *layout =
                Layout::array::<u8>(span.len()).map_err(|_| AllocationRefusal::DemandOverflow)?;
        }
        layouts[4] = ChargedShared::<ChargedCertificateParts>::allocation_layout();
        Ok(layouts)
    }
    /// Whether every actual or awaiting allocation retains this original finite pool.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.budget.same_pool(budget)
            && self
                .values
                .iter()
                .flatten()
                .all(|value| value.belongs_to(budget))
            && self
                .charges
                .iter()
                .flatten()
                .all(|charge| charge.belongs_to(budget))
            && self
                .shell
                .as_ref()
                .is_none_or(|shell| shell.belongs_to(budget))
    }
    /// Borrow completed backing prefixes for original-source retry diagnostics.
    /// A prefix alone establishes no frame, signature, quorum or result authority.
    ///
    /// # Errors
    /// Rejects any change to the complete original input or its pool.
    pub fn initialized<'a>(
        &'a self,
        input: &ChargedBuffer<u8>,
    ) -> Result<[Option<&'a [u8]>; 4], CertificateCustodyError> {
        self.check(input)?;
        Ok(std::array::from_fn(|index| {
            self.values[index].as_ref().map(ChargedBuffer::as_slice)
        }))
    }
    /// Allocate every prepaid original child, then fill through the canonical byte kernel.
    ///
    /// # Errors
    /// Original pool, allocator, logical scope and source causes retain all siblings/credits.
    pub fn prepare(&mut self, input: &ChargedBuffer<u8>) -> Result<(), CertificateCustodyError> {
        self.check(input)?;
        let flags =
            norito::core::effective_decode_flags().ok_or(CertificateCustodyError::MissingLayout)?;
        if flags != self.flags {
            return Err(CertificateCustodyError::LayoutChanged);
        }
        if self.ready {
            return Ok(());
        }
        if !self.admitted {
            let layouts = self.allocation_layouts()?;
            let mut reservation = self.budget.try_reserve_layouts(layouts)?;
            for (charge, layout) in self.charges.iter_mut().zip(layouts) {
                *charge = Some(
                    reservation
                        .try_split(layout)
                        .expect("exact combined layouts admitted before allocation"),
                );
            }
            self.admitted = true;
        }
        for index in 0..4 {
            if self.values[index].is_none() {
                let charge = self.charges[index]
                    .take()
                    .expect("same prepaid original leaf charge");
                match ChargedBuffer::try_from_charge(self.leaves[index].len(), charge) {
                    Ok(buffer) => self.values[index] = Some(buffer),
                    Err((charge, error)) => {
                        self.charges[index] = Some(charge);
                        return Err(error.into());
                    }
                }
            }
        }
        if self.shell.is_none() {
            let charge = self.charges[4]
                .take()
                .expect("same prepaid original control charge");
            match ChargedShared::reserve_from_charge(charge) {
                Ok(shell) => self.shell = Some(shell),
                Err((charge, error)) => {
                    self.charges[4] = Some(charge);
                    return Err(error.into());
                }
            }
        }
        // All physical destinations exist before exposing any decoded certificate bytes.
        for value in self.values.iter_mut().flatten() {
            while value.as_slice().len() < value.capacity() {
                value.push_reserved(0);
            }
        }
        let bytes = self
            .span
            .get(input.as_slice())
            .map_err(|_| CertificateCustodyError::SourceRange)?;
        norito::core::classify_decode_attempt(|| {
            let mut fields = Fill {
                values: &mut self.values,
            };
            let (_, used) = CanonicalParts::decode_fields(bytes, &mut fields)
                .map_err(DecodeIntoError::into_codec)?;
            if used != bytes.len() {
                return Err(norito::Error::LengthMismatch);
            }
            Ok(())
        })?;
        self.ready = true;
        Ok(())
    }
    /// Move these complete original leaves into the already allocated immutable shell.
    ///
    /// # Errors
    /// Returns this same owner if its source changed or any canonical fill is incomplete.
    #[expect(
        clippy::result_large_err,
        reason = "refusal returns all original children and prepaid charges without allocation"
    )]
    pub fn finish(
        mut self,
        input: &ChargedBuffer<u8>,
    ) -> Result<CommitCertificate, (Self, CertificateCustodyError)> {
        if let Err(error) = self.check(input) {
            return Err((self, error));
        }
        if !self.ready || self.shell.is_none() {
            return Err((self, CertificateCustodyError::Incomplete));
        }
        let [consensus_header, commit_qc, result_preimage, availability] =
            std::array::from_fn(|index| {
                self.values[index]
                    .take()
                    .expect("complete canonical original leaf")
            });
        let parts = ChargedCertificateParts {
            consensus_header,
            commit_qc,
            result_preimage,
            availability,
        };
        let owner = self
            .shell
            .take()
            .expect("complete original shared shell")
            .initialize(parts);
        Ok(CommitCertificate {
            storage: Storage::Admitted(owner),
        })
    }
}
