//! Opaque available-body custody; no detached validation flag can substitute for exact owners.
use super::{
    AcquisitionError, AvailabilityError, AvailabilityFrame, PayloadBytes, VerifiedManifest,
};
use crate::{
    bytes::ByteAdmissionError,
    crypto::Crypto,
    message::BlockHeader,
    types::{Hash32, HeightConfig},
};
use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_primitives::erasure::rs16::compact::{
    CodecAllocationError, encode_funded, reconstruct_funded,
};

/// Immutable original-funded body usable after the existing durable-body barrier.
/// Construction consumes exact authenticated reconstruction; cloning shares the backing.
/// This is not a wire decoder or a substitute for Core's height/proposal/finality checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableBody {
    source: AvailabilitySource,
    header: BlockHeader,
    frame: AvailabilityFrame,
    payload: PayloadBytes,
}
impl AvailableBody {
    /// Identity of the exact authenticated header retained with this body.
    pub fn hash(&self, crypto: &dyn Crypto) -> Hash32 {
        self.header.hash(crypto)
    }
    // Only the sibling original-author worker may supply its privately retained encoded source.
    pub(super) fn authored(
        source: AvailabilitySource,
        header: BlockHeader,
        frame: AvailabilityFrame,
        payload: PayloadBytes,
    ) -> Self {
        Self {
            source,
            header,
            frame,
            payload,
        }
    }
    /// Full immutable authority under which these exact bytes and original signatures passed.
    /// Publication and Core acceptance compare it with their independently selected source.
    pub fn source(&self) -> &AvailabilitySource {
        &self.source
    }
    /// Header whose identity every original availability signature binds.
    pub fn header(&self) -> &BlockHeader {
        &self.header
    }
    /// Complete original signature table, retained unchanged for publication and later serving.
    pub fn availability(&self) -> &AvailabilityFrame {
        &self.frame
    }
    /// Exact reconstructed application payload with its original backing owner.
    pub fn payload(&self) -> &PayloadBytes {
        &self.payload
    }
    /// Both retained bulk owners must belong to the same production pool.
    pub fn admitted_to(&self, budget: &AllocationBudget) -> bool {
        self.frame.admitted_to(budget) && self.payload.admitted_to(budget)
    }
}
/// Verified output awaiting only a shared-control allocation. A refusal returns this same
/// move-only owner; it neither reconstructs again nor copies a payload into an uncharged Vec.
pub struct VerifiedMaterial {
    source: AvailabilitySource,
    header: BlockHeader,
    frame: AvailabilityFrame,
    payload: ChargedBuffer<u8>,
}
impl VerifiedMaterial {
    /// Finish immutable sharing or return the exact original output for a later resource retry.
    #[allow(
        clippy::result_large_err,
        reason = "retain original metadata and charged backing"
    )]
    pub fn finish(
        self,
        budget: &AllocationBudget,
    ) -> Result<AvailableBody, (Self, ByteAdmissionError)> {
        let Self {
            source,
            header,
            frame,
            payload,
        } = self;
        if !frame.admitted_to(budget) {
            return Err((
                Self {
                    source,
                    header,
                    frame,
                    payload,
                },
                ByteAdmissionError::ForeignBudget,
            ));
        }
        match PayloadBytes::from_charged(payload, budget) {
            Ok(payload) => Ok(AvailableBody {
                source,
                header,
                frame,
                payload,
            }),
            Err((payload, error)) => Err((
                Self {
                    source,
                    header,
                    frame,
                    payload,
                },
                error,
            )),
        }
    }
}
impl VerifiedManifest<'_> {
    fn source(&self) -> AvailabilitySource {
        AvailabilitySource {
            instance: self.table.header.instance,
            height: self.table.header.height,
            block_hash: self.table.block_hash,
            config: self.config.clone(),
        }
    }
    /// The single authenticated reconstruction-to-custody boundary. The codec checks
    /// received-row consistency and padding, and this exact manifest checks every regenerated
    /// row once before its payload can enter the opaque retained owner. No caller-supplied
    /// reconstruction or detached verification result can bypass that transition.
    pub(super) fn reconstruct(
        &self,
        received: &[Option<&[u8]>],
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> Result<VerifiedMaterial, AcquisitionError> {
        if !self.frame.admitted_to(budget) {
            return Err(AcquisitionError::Bytes(ByteAdmissionError::ForeignBudget));
        }
        let reconstructed = reconstruct_funded(self.table.shape, received, budget, |index, row| {
            self.accepts_row(index, row, crypto)
        })
        .map_err(AcquisitionError::Codec)?;
        self.table
            .check_payload(reconstructed.payload(), crypto)
            .map_err(AcquisitionError::Manifest)?;
        Ok(VerifiedMaterial {
            source: self.source(),
            header: self.table.header.clone(),
            frame: self.frame.clone(),
            payload: reconstructed.into_payload(),
        })
    }
}

/// Restoration failure separates a signed defect from a local codec resource refusal.
#[derive(Debug)]
pub enum RestorationError {
    /// Original-pool frame or payload admission could not complete.
    Bytes(ByteAdmissionError),
    /// Stored input differs from the original authenticated evidence.
    Invalid(AvailabilityError),
    /// Original-pool transient RS16 backing could not be acquired.
    Codec(CodecAllocationError),
}

impl RestorationError {
    /// Only local resource refusal permits retry of the same original job.
    pub fn is_local_refusal(&self) -> bool {
        match self {
            Self::Bytes(error) => error.is_local_refusal(),
            Self::Codec(
                CodecAllocationError::Admission(_) | CodecAllocationError::Allocation(_),
            ) => true,
            Self::Invalid(_) | Self::Codec(CodecAllocationError::Codec(_)) => false,
        }
    }
}
/// Immutable independent acquisition expectation. The caller supplies `config` from the
/// authenticated historical schedule, never from the stored artifact being checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailabilitySource {
    instance: Hash32,
    height: u64,
    block_hash: Hash32,
    config: HeightConfig,
}
impl AvailabilitySource {
    /// Bind one expected identity to its already authenticated complete height authority.
    pub fn new(
        instance: Hash32,
        height: u64,
        block_hash: Hash32,
        config: HeightConfig,
    ) -> Result<Self, AvailabilityError> {
        if !config.epoch.contains(height) {
            return Err(AvailabilityError::Source);
        }
        Ok(Self {
            instance,
            height,
            block_hash,
            config,
        })
    }
    /// Independently selected consensus instance.
    pub fn instance(&self) -> Hash32 {
        self.instance
    }
    /// Independently requested historical height.
    pub fn height(&self) -> u64 {
        self.height
    }
    /// Independently requested block hash.
    pub fn block_hash(&self) -> Hash32 {
        self.block_hash
    }
    /// Exact authoritative configuration used for every signature and geometry decision.
    pub fn config(&self) -> &HeightConfig {
        &self.config
    }
    pub(super) fn check_header(
        &self,
        header: &BlockHeader,
        crypto: &dyn Crypto,
    ) -> Result<(), AvailabilityError> {
        if header.instance != self.instance
            || header.height != self.height
            || header.epoch != self.config.epoch.id
            || header.skipped_leaders.len() > self.config.committee.n()
            || header.hash(crypto) != self.block_hash
        {
            return Err(AvailabilityError::Source);
        }
        Ok(())
    }
}
/// Move-only storage-to-worker handoff. Decoding supplies owners, never availability authority.
/// This job retains every already-admitted frame/payload backing through a local refusal.
pub struct BodyRestoration {
    source: AvailabilitySource,
    header: BlockHeader,
    frame: AvailabilityFrame,
    payload: PayloadBytes,
}
impl BodyRestoration {
    /// Retain exact untrusted decoded inputs and an independently authenticated expectation.
    pub fn new(
        source: AvailabilitySource,
        header: BlockHeader,
        frame: AvailabilityFrame,
        payload: PayloadBytes,
    ) -> Self {
        Self {
            source,
            header,
            frame,
            payload,
        }
    }
    /// Immutable original expectation retained across every retry.
    pub fn source(&self) -> &AvailabilitySource {
        &self.source
    }
    /// Complete on a worker; success consumes exact original owners, refusal returns this job.
    #[allow(
        clippy::result_large_err,
        reason = "retain exact source and original byte owners"
    )]
    pub fn complete(
        mut self,
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> Result<AvailableBody, (Self, RestorationError)> {
        if let Err(error) = self.prepare(budget, crypto) {
            return Err((self, error));
        }
        Ok(AvailableBody {
            source: self.source,
            header: self.header,
            frame: self.frame,
            payload: self.payload,
        })
    }
    fn prepare(
        &mut self,
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> Result<(), RestorationError> {
        self.source
            .check_header(&self.header, crypto)
            .map_err(RestorationError::Invalid)?;
        self.frame.admit(budget).map_err(RestorationError::Bytes)?;
        self.payload
            .admit(budget)
            .map_err(RestorationError::Bytes)?;
        let manifest = super::verify_manifest(
            self.source.instance,
            &self.source.config,
            &self.header,
            &self.frame,
            budget,
            crypto,
        )
        .map_err(RestorationError::Invalid)?;
        // Successful in-place admission above binds both original owners to this pool.
        // The sole restoration path then checks actual payload and every re-encoded row.
        manifest
            .table
            .check_payload(self.payload.as_slice(), crypto)
            .map_err(RestorationError::Invalid)?;
        let encoded = encode_funded(manifest.table.shape, self.payload.as_slice(), budget)
            .map_err(RestorationError::Codec)?;
        manifest
            .table
            .check_codeword(encoded.codeword(), crypto)
            .map_err(RestorationError::Invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Fixture;
    use super::*;
    #[test]
    fn checked_custody_refusals_preserve_exact_payload_and_frame_owners() {
        let f = Fixture::new();
        let manifest = f.verify().unwrap();
        let shape = f.encoded.shape();
        let received: Vec<_> = (0..shape.chunk_count())
            .map(|i| (i % 3 < 2).then(|| &f.encoded.codeword()[shape.chunk_range(i).unwrap()]))
            .collect();
        let material = manifest
            .reconstruct(&received, &f.budget, &f.keys.crypto)
            .unwrap_or_else(|_| panic!("valid original reconstruction"));
        let payload_pointer = material.payload.as_slice().as_ptr();
        let reserve = f
            .budget
            .try_reserve_bytes((1 << 20) - f.budget.reserved_bytes())
            .unwrap();
        let (material, error) = material.finish(&f.budget).unwrap_err();
        assert!(matches!(
            error,
            crate::bytes::ByteAdmissionError::ControlAdmission(_)
        ));
        let (material, error) = material
            .finish(&AllocationBudget::new(1 << 20))
            .unwrap_err();
        assert!(matches!(
            error,
            crate::bytes::ByteAdmissionError::ForeignBudget
        ));
        drop(reserve);
        let body = match material.finish(&f.budget) {
            Ok(body) => body,
            Err(_) => panic!("same pool retry"),
        };
        assert_eq!(body.header(), &f.header);
        assert_eq!(body.payload().as_slice(), f.payload);
        assert_eq!(body.payload().as_slice().as_ptr(), payload_pointer);
        assert_eq!(
            body.availability().as_slice().as_ptr(),
            f.frame.as_slice().as_ptr()
        );
        let before = f.budget.reserved_bytes();
        let clone = body.clone();
        assert_eq!(f.budget.reserved_bytes(), before);
        assert_eq!(clone.payload().as_slice().as_ptr(), payload_pointer);
        assert!(clone.admitted_to(&f.budget));
        assert!(!clone.admitted_to(&AllocationBudget::new(1 << 20)));
    }
}
