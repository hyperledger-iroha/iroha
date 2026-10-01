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
use std::borrow::Borrow;

/// Immutable original-funded body usable after the existing durable-body barrier.
/// Construction consumes exact authenticated reconstruction. A custom source owner may retain
/// independently funded configuration custody; it is moved unchanged through restoration.
/// Bulk backing is shared on clone; cloning metadata/source fields remains caller-owned work.
/// This is not a wire decoder or a substitute for Core's height/proposal/finality checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvailableBody<Source = AvailabilitySource> {
    source: Source,
    header: BlockHeader,
    frame: AvailabilityFrame,
    payload: PayloadBytes,
}
impl<Source: Borrow<AvailabilitySource>> AvailableBody<Source> {
    /// Identity of the exact authenticated header retained with this body.
    pub fn hash(&self, crypto: &dyn Crypto) -> Hash32 {
        self.header.hash(crypto)
    }
    /// Full immutable authority under which these exact bytes and original signatures passed.
    /// Publication and Core acceptance compare it with their independently selected source.
    pub fn source(&self) -> &AvailabilitySource {
        self.source.borrow()
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
impl AvailableBody {
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
    ///
    /// # Errors
    /// Returns the original material if its frame or payload belongs to another pool, or
    /// the payload cannot acquire its shared owner in the supplied pool.
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
            if cfg!(sumeragi_mutation = "MS20d") {
                self.table
                    .shape
                    .chunk_range(index)
                    .is_some_and(|range| range.len() == row.len())
            } else {
                self.accepts_row(index, row, crypto)
            }
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
    ///
    /// # Errors
    /// Rejects a height outside the supplied authenticated epoch.
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
        if header.skipped_leaders.len() > self.config.committee.n()
            || (!cfg!(sumeragi_mutation = "MS20b")
                && (header.instance != self.instance
                    || header.height != self.height
                    || header.epoch != self.config.epoch.id
                    || header.hash(crypto) != self.block_hash))
        {
            return Err(AvailabilityError::Source);
        }
        Ok(())
    }
}
/// Move-only storage-to-worker handoff. Decoding supplies owners, never availability authority.
/// This job retains every already-admitted frame/payload backing and its complete source owner
/// through a local refusal. The source owner need not be Clone; no extraction API detaches it.
pub struct BodyRestoration<Source = AvailabilitySource> {
    source: Source,
    header: BlockHeader,
    frame: AvailabilityFrame,
    payload: PayloadBytes,
}
impl<Source: Borrow<AvailabilitySource>> BodyRestoration<Source> {
    /// Retain exact untrusted decoded inputs and an independently authenticated expectation.
    pub fn new(
        source: Source,
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
        self.source.borrow()
    }
    /// Complete on a worker; success consumes exact original owners, refusal returns this job.
    ///
    /// # Errors
    /// Returns the retained job if original source, geometry, signatures, payload or codeword
    /// checks fail, funding is foreign, or admission or codec allocation is refused.
    #[allow(
        clippy::result_large_err,
        reason = "retain exact source and original byte owners"
    )]
    pub fn complete(
        mut self,
        budget: &AllocationBudget,
        crypto: &dyn Crypto,
    ) -> Result<AvailableBody<Source>, (Self, RestorationError)> {
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
        let source = self.source.borrow();
        source
            .check_header(&self.header, crypto)
            .map_err(RestorationError::Invalid)?;
        self.frame.admit(budget).map_err(RestorationError::Bytes)?;
        self.payload
            .admit(budget)
            .map_err(RestorationError::Bytes)?;
        let manifest = super::verify_manifest(
            source.instance,
            &source.config,
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
    // Intentionally move-only: returning the source requires moving the exact original owner.
    struct MoveOnlySource {
        source: AvailabilitySource,
        token: ChargedBuffer<u8>,
        drops: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }
    impl Borrow<AvailabilitySource> for MoveOnlySource {
        fn borrow(&self) -> &AvailabilitySource {
            &self.source
        }
    }
    impl Drop for MoveOnlySource {
        fn drop(&mut self) {
            self.drops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    fn move_only_restoration(
        f: &Fixture,
    ) -> (
        BodyRestoration<MoveOnlySource>,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) {
        let shape = f.encoded.shape();
        let received: Vec<_> = (0..shape.chunk_count())
            .map(|index| Some(&f.encoded.codeword()[shape.chunk_range(index).unwrap()]))
            .collect();
        let body = f
            .verify()
            .unwrap()
            .reconstruct(&received, &f.budget, &f.keys.crypto)
            .unwrap_or_else(|_| panic!("valid original reconstruction"))
            .finish(&f.budget)
            .unwrap_or_else(|_| panic!("valid original custody"));
        let AvailableBody {
            source,
            header,
            frame,
            payload,
        } = body;
        let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        // This token proves source ownership; it does not claim to fund fixture config clones.
        let mut token = ChargedBuffer::new(17, &f.budget).unwrap();
        token.append(b"same source owner").unwrap();
        (
            BodyRestoration::new(
                MoveOnlySource {
                    source,
                    token,
                    drops: drops.clone(),
                },
                header,
                frame,
                payload,
            ),
            drops,
        )
    }

    #[test]
    fn restoration_moves_original_source_owner_through_refusal_retry_and_drop() {
        let f = Fixture::new();
        let baseline = f.budget.reserved_bytes();
        let (request, drops) = move_only_restoration(&f);
        let token_pointer = request.source.token.as_slice().as_ptr();
        let payload_pointer = request.payload.as_slice().as_ptr();
        let frame_pointer = request.frame.as_slice().as_ptr();
        let epoch_pointer = std::ptr::from_ref(request.source().config().epoch.as_ref());
        let reserved = f.budget.reserved_bytes();
        let foreign = AllocationBudget::new(1 << 20);
        let (request, error) = request
            .complete(&foreign, &f.keys.crypto)
            .err()
            .expect("foreign pool");
        assert!(matches!(
            error,
            RestorationError::Bytes(ByteAdmissionError::ForeignBudget)
        ));
        assert_eq!(foreign.reserved_bytes(), 0);
        assert_eq!(f.budget.reserved_bytes(), reserved);
        f.budget.set_limit_bytes(reserved);
        let (request, error) = request
            .complete(&f.budget, &f.keys.crypto)
            .err()
            .expect("no scratch capacity");
        assert!(error.is_local_refusal());
        assert_eq!(request.source.token.as_slice().as_ptr(), token_pointer);
        assert_eq!(request.payload.as_slice().as_ptr(), payload_pointer);
        assert_eq!(request.frame.as_slice().as_ptr(), frame_pointer);
        assert_eq!(
            std::ptr::from_ref(request.source().config().epoch.as_ref()),
            epoch_pointer
        );
        assert_eq!(f.budget.reserved_bytes(), reserved);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        f.budget.set_limit_bytes(1 << 20);
        let body = request
            .complete(&f.budget, &f.keys.crypto)
            .unwrap_or_else(|(_, error)| panic!("original pool retry: {error:?}"));
        assert_eq!(body.source.token.as_slice().as_ptr(), token_pointer);
        assert_eq!(body.payload().as_slice().as_ptr(), payload_pointer);
        assert_eq!(body.availability().as_slice().as_ptr(), frame_pointer);
        assert_eq!(
            std::ptr::from_ref(body.source().config().epoch.as_ref()),
            epoch_pointer
        );
        assert_eq!(body.header(), &f.header);
        assert_eq!(body.source().config(), &f.config);
        assert!(body.admitted_to(&f.budget));
        assert_eq!(f.budget.reserved_bytes(), reserved);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        drop(body);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(f.budget.reserved_bytes(), baseline);
    }

    #[test]
    fn restoration_returns_original_source_owner_on_invalid_header() {
        let f = Fixture::new();
        let baseline = f.budget.reserved_bytes();
        let (mut request, drops) = move_only_restoration(&f);
        let pointer = request.source.token.as_slice().as_ptr();
        request.header.height += 1;
        let reserved = f.budget.reserved_bytes();
        let (request, error) = request
            .complete(&f.budget, &f.keys.crypto)
            .err()
            .expect("substituted header");
        assert!(matches!(
            error,
            RestorationError::Invalid(AvailabilityError::Source)
        ));
        assert!(!error.is_local_refusal());
        assert_eq!(request.source.token.as_slice().as_ptr(), pointer);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(f.budget.reserved_bytes(), reserved);
        drop(request);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(f.budget.reserved_bytes(), baseline);
    }

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
        let body = material
            .finish(&f.budget)
            .unwrap_or_else(|(_, error)| panic!("same pool retry: {error}"));
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
