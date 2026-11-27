//! Exact finalized seed bytes enter only the existing isolated publisher-source staging owner.

use super::*;
use iroha_core::query::provider_ingest_source::authorize_publisher_source_v1;
use sorafs_car::{
    publisher::{
        PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1, PublisherSourceChunkRequestV1,
        PublisherSourceHeaderV1, PublisherSourceUploadV1,
    },
    verifier::CarVerifier,
};
use sorafs_node::store::StorageError;
use std::io::{self, Read as _};

impl NativeMusubiStorageBackendV1 {
    pub(super) fn stage(
        &self,
        request: &MusubiStorageCoordinationRequestV1,
        pin: &NativeMusubiFinalizedPinV1,
        assignment: &SeedStagingAssignment,
        progress: PublisherSourceProgressV1,
        authorization: &NativePinAuthorizationV1,
        deadline: Instant,
    ) -> Result<()> {
        let binding = &assignment.binding;
        authorization.check_effect_boundary(&mut ClockSample(&self.clock), deadline)?;
        let seed = self
            .seed
            .read_finalized_seed(&pin.query().source)
            .map_err(|_| eyre::eyre!("original finalized seed custody unavailable"))?;
        let manifest = super::super::pin_registration::validate_signed_pin_intent(
            &request.network_id,
            &self.authority,
            &request.commitment,
            &pin.query().transaction,
            pin.query().manifest_digest,
        )?;
        // The sole header/verifier own their structural algorithms. Reserve their complete
        // bounded work before cloning any plan/header or materializing a payload view.
        norito::core::reserve_decode_allocation(
            PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1
                .checked_mul(8)
                .ok_or_else(|| eyre::eyre!("publisher metadata allowance overflow"))?,
        )?;
        let header = PublisherSourceHeaderV1::new(
            *binding.provider_id.as_bytes(),
            *binding.order_id.as_bytes(),
            binding.assignment_revision,
            &manifest,
            seed.plan(),
        )?;
        // Never treat CAR offsets as payload offsets or allocate a second whole payload.
        norito::core::reserve_decode_allocation(
            seed.car()
                .len()
                .checked_mul(2)
                .ok_or_else(|| eyre::eyre!("canonical CAR work allowance overflow"))?,
        )?;
        let verified =
            CarVerifier::verify_canonical_car_with_plan_retained(seed.plan(), seed.car())?;
        let mut payload = verified.payload_reader();
        let index = match progress {
            PublisherSourceProgressV1::Available => return Ok(()),
            PublisherSourceProgressV1::NeedsHeader => {
                let check_deadline = || {
                    authorization
                        .check_effect_boundary(&mut ClockSample(&self.clock), deadline)
                        .map_err(|_| {
                            StorageError::Io(io::Error::other("publisher source grant expired"))
                        })
                };
                let current_authority = || {
                    let now = check_deadline()?;
                    let accepted = authorize_publisher_source_v1(
                        &self.state.view(),
                        &self.authority,
                        binding,
                        now / 1_000,
                    )
                    .map_err(|_| {
                        StorageError::Io(io::Error::other("publisher source authority unavailable"))
                    })?;
                    if accepted.deadline_epoch != assignment.deadline_epoch {
                        return Err(StorageError::Io(io::Error::other(
                            "publisher source deadline changed",
                        )));
                    }
                    Ok(accepted.finalized_epoch)
                };
                let published = self.storage.stage_complete_publisher_source(
                    &header,
                    assignment.deadline_epoch,
                    &mut payload,
                    &current_authority,
                    || check_deadline().map(|_| ()),
                )?;
                if !published {
                    // A concurrently staged or superseded assignment stays with the
                    // established header owner, which preserves matching partial uploads
                    // and replaces only a strictly older authorized revision.
                    let epoch = current_authority()?;
                    self.storage.stage_publisher_source(
                        &header,
                        assignment.deadline_epoch,
                        epoch,
                    )?;
                    current_authority()?;
                }
                return Ok(());
            }
            PublisherSourceProgressV1::NeedsChunk(index) => index,
        };
        let chunk = header
            .chunks
            .get(index)
            .ok_or_else(|| eyre::eyre!("missing chunk is outside the retained source"))?;
        let length = chunk.length as usize;
        ensure!(
            length > 0 && length <= sorafs_car::CHUNK_STORE_MAX_CHUNK_BYTES as usize,
            "publisher chunk extent is invalid"
        );
        // The verified stream presents concatenated payload bytes. CAR framing offsets
        // and original filesystem paths are never used to recover a missing chunk.
        let skipped = std::io::copy(
            &mut payload.by_ref().take(chunk.offset),
            &mut std::io::sink(),
        )?;
        ensure!(
            skipped == chunk.offset,
            "publisher chunk starts beyond its payload"
        );
        norito::core::reserve_decode_allocation(length)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length)?;
        bytes.resize(length, 0);
        payload.read_exact(&mut bytes)?;
        ensure!(
            blake3::hash(&bytes).as_bytes() == &chunk.digest,
            "publisher chunk digest differs"
        );
        let now = authorization.check_effect_boundary(&mut ClockSample(&self.clock), deadline)?;
        let accepted = authorize_publisher_source_v1(
            &self.state.view(),
            &self.authority,
            binding,
            now / 1_000,
        )?;
        let upload = PublisherSourceChunkRequestV1 {
            provider_id: *binding.provider_id.as_bytes(),
            order_id: *binding.order_id.as_bytes(),
            assignment_revision: binding.assignment_revision,
            manifest_digest: *binding.manifest_digest.as_bytes(),
            header_digest: header.canonical_digest()?,
            upload: PublisherSourceUploadV1 {
                index: u32::try_from(index)?,
                bytes,
            },
        };
        self.storage
            .stage_publisher_source_chunk(&upload, accepted.finalized_epoch)?;
        let now = authorization.check_effect_boundary(&mut ClockSample(&self.clock), deadline)?;
        authorize_publisher_source_v1(&self.state.view(), &self.authority, binding, now / 1_000)?;
        Ok(())
    }
}
