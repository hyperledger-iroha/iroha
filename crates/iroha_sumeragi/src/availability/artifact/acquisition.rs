//! Source-bound actual-row acquisition. Signature-table possession never counts as custody.
use super::{
    AvailabilityError, AvailabilitySource, AvailableBody, RowBytes, VerifiedAvailability,
    VerifiedManifest, VerifiedMaterial, verify_manifest,
};
use crate::{
    bytes::ByteAdmissionError,
    crypto::Crypto,
    message::{PayloadChunk, PayloadManifest},
};
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_primitives::erasure::rs16::compact::{CodecAllocationError, CompactShape};

/// Acquisition cannot confuse missing rows, a bad relay, signed invalidity and local refusal.
#[derive(Debug)]
pub enum AcquisitionError {
    /// At least one stripe still lacks k distinct actually received rows.
    Incomplete,
    /// The original manifest does not match its independently selected source or signatures.
    Manifest(AvailabilityError),
    /// An unsigned relay chunk does not match the original manifest; never a leader fault.
    Row(AvailabilityError),
    /// Original-pool byte custody admission refused; the original owner is retained.
    Bytes(ByteAdmissionError),
    /// Original-pool metadata slots or transient references could not be allocated.
    Slots(ChargedBufferError),
    /// Actual reconstruction either refused local resources or disproved the signed codeword.
    Codec(CodecAllocationError),
}
impl AcquisitionError {
    /// Whether unchanged original owners can be retried after local resource recovery.
    pub fn is_local_refusal(&self) -> bool {
        match self {
            Self::Bytes(error) => error.is_local_refusal(),
            Self::Slots(_)
            | Self::Codec(
                CodecAllocationError::Admission(_) | CodecAllocationError::Allocation(_),
            ) => true,
            _ => false,
        }
    }
    /// Whether this carrier is invalid, independent of a relay's bad or missing chunk.
    pub fn rejects_manifest(&self) -> bool {
        matches!(
            self,
            Self::Manifest(_) | Self::Codec(CodecAllocationError::Codec(_))
        )
    }
}

/// Move-only acquisition state retained by a bounded worker, never by a wire decoder.
/// All retained actual row slots/backings and reconstruction outputs use the original pool.
pub struct PayloadAcquisition {
    source: AvailabilitySource,
    manifest: PayloadManifest,
    shape: Option<CompactShape>,
    rows: Option<ChargedBuffer<Option<RowBytes>>>,
    material: Option<VerifiedMaterial>,
}
impl PayloadAcquisition {
    /// Retain an untrusted manifest under the independently authenticated expected source.
    pub fn new(source: AvailabilitySource, manifest: PayloadManifest) -> Self {
        Self {
            source,
            manifest,
            shape: None,
            rows: None,
            material: None,
        }
    }
    /// Immutable expectation for exact worker-job matching and historical authority.
    pub fn source(&self) -> &AvailabilitySource {
        &self.source
    }
    /// Exact original carrier for duplicate-job matching or a typed rejection event.
    pub fn manifest(&self) -> &PayloadManifest {
        &self.manifest
    }
    /// Actual distinct row count; the manifest's authorization table contributes zero.
    pub fn received_rows(&self) -> usize {
        self.rows.as_ref().map_or(0, |rows| {
            rows.as_slice().iter().filter(|row| row.is_some()).count()
        })
    }
    /// Verify all original signatures once, then allocate exact bounded row-owner slots.
    /// Every refusal preserves this job and every already admitted backing allocation.
    ///
    /// # Errors
    /// Rejects invalid manifest authority, geometry, content or signatures, foreign funding,
    /// or refusal to admit the manifest or allocate row-owner slots.
    pub fn prepare(
        &mut self,
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> Result<(), AcquisitionError> {
        self.manifest
            .availability
            .admit(budget)
            .map_err(AcquisitionError::Bytes)?;
        if self.shape.is_none() {
            self.source
                .check_header(&self.manifest.header, crypto)
                .map_err(AcquisitionError::Manifest)?;
            self.shape = Some(
                verify_manifest(
                    self.source.instance(),
                    self.source.config(),
                    &self.manifest.header,
                    &self.manifest.availability,
                    budget,
                    crypto,
                )
                .map_err(AcquisitionError::Manifest)?
                .table
                .shape,
            );
        }
        if self.material.is_some() {
            return Ok(());
        }
        if let Some(rows) = &self.rows {
            if !rows.belongs_to(budget) {
                return Err(AcquisitionError::Bytes(ByteAdmissionError::ForeignBudget));
            }
        } else {
            let count = self.shape.expect("verified shape").chunk_count();
            let mut rows = ChargedBuffer::new(count, budget).map_err(AcquisitionError::Slots)?;
            for _ in 0..count {
                rows.push_reserved(None);
            }
            self.rows = Some(rows);
        }
        Ok(())
    }
    fn verified(&self) -> VerifiedManifest<'_> {
        VerifiedManifest {
            table: VerifiedAvailability {
                header: &self.manifest.header,
                bytes: self.manifest.availability.as_slice(),
                shape: self.shape.expect("prepare authenticated immutable fields"),
                block_hash: self.source.block_hash(),
            },
            frame: &self.manifest.availability,
            config: self.source.config(),
        }
    }
    /// Admit and retain one actual distinct row. On error return the exact original chunk.
    /// `false` means a duplicate, which never advances reconstruction readiness.
    ///
    /// # Errors
    /// Returns the original chunk on manifest preparation failure, mismatched row source
    /// or commitment, or refusal to admit its bytes to the original pool.
    #[allow(
        clippy::result_large_err,
        reason = "return the actual received byte owner"
    )]
    pub fn push(
        &mut self,
        mut chunk: PayloadChunk,
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> Result<bool, (PayloadChunk, AcquisitionError)> {
        let accept = |this: &mut Self, chunk: &mut PayloadChunk| {
            this.prepare(budget, crypto)?;
            if chunk.instance != this.source.instance()
                || chunk.height != this.source.height()
                || chunk.block_hash != this.source.block_hash()
            {
                return Err(AcquisitionError::Row(AvailabilityError::Source));
            }
            if !this
                .verified()
                .accepts_row(chunk.index as usize, chunk.bytes.as_slice(), crypto)
            {
                return Err(AcquisitionError::Row(AvailabilityError::Digest));
            }
            chunk.bytes.admit(budget).map_err(AcquisitionError::Bytes)?;
            Ok(())
        };
        if let Err(error) = accept(self, &mut chunk) {
            return Err((chunk, error));
        }
        if self.material.is_some() {
            return Ok(false);
        }
        let slot =
            &mut self.rows.as_mut().expect("prepared slots").as_mut_slice()[chunk.index as usize];
        if slot.is_some() {
            return Ok(false);
        }
        *slot = Some(chunk.bytes);
        Ok(true)
    }
    /// Reconstruct only after every stripe has k distinct authenticated actual rows.
    /// A local refusal returns this exact job, including completed material awaiting sharing.
    ///
    /// # Errors
    /// Returns the retained job if preparation fails, any stripe has too few distinct rows,
    /// reconstruction or commitment checks fail, or an allocation or sharing request is refused.
    #[allow(
        clippy::result_large_err,
        reason = "retain source, received owners and completed output"
    )]
    pub fn complete(
        mut self,
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> Result<AvailableBody, (Self, AcquisitionError)> {
        if let Err(error) = self.reconstruct(budget, crypto) {
            return Err((self, error));
        }
        match self
            .material
            .take()
            .expect("reconstructed custody")
            .finish(budget)
        {
            Ok(body) => Ok(body),
            Err((material, error)) => {
                self.material = Some(material);
                Err((self, AcquisitionError::Bytes(error)))
            }
        }
    }
    fn reconstruct(
        &mut self,
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> Result<(), AcquisitionError> {
        self.prepare(budget, crypto)?;
        if self.material.is_some() {
            return Ok(());
        }
        let layout = self.source.config().epoch.da_layout;
        let rows = self.rows.as_ref().expect("prepared slots").as_slice();
        let width = usize::from(layout.data_shards) + usize::from(layout.parity_shards);
        if rows.chunks_exact(width).any(|stripe| {
            stripe.iter().filter(|row| row.is_some()).count() < usize::from(layout.data_shards)
        }) {
            return Err(AcquisitionError::Incomplete);
        }
        let mut refs = ChargedBuffer::new(rows.len(), budget).map_err(AcquisitionError::Slots)?;
        for row in rows {
            refs.push_reserved(row.as_ref().map(crate::bytes::ByteSequence::as_slice));
        }
        let verified = self.verified();
        let material = verified.reconstruct(refs.as_slice(), budget, crypto)?;
        drop(refs);
        self.rows = None;
        self.material = Some(material);
        Ok(())
    }
}
