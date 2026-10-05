//! Exact finalized seed bytes enter only the existing isolated publisher-source staging owner.

use super::*;
use iroha_core::query::provider_ingest_source::{
    PublisherSourceBindingV1, authorize_publisher_source_v1,
};
use sorafs_car::{
    publisher::{
        PUBLISHER_SOURCE_HEADER_MAX_BYTES_V1, PublisherSourceChunkRequestV1,
        PublisherSourceHeaderV1, PublisherSourceUploadV1,
    },
    verifier::CarVerifier,
};
use std::io::Read as _;

impl NativeMusubiStorageBackendV1 {
    pub(super) fn stage(
        &self,
        request: &MusubiStorageCoordinationRequestV1,
        pin: &NativeMusubiFinalizedPinV1,
        authorization: &NativePinAuthorizationV1,
        deadline: Instant,
    ) -> Result<()> {
        let now = authorization.check_effect_boundary(&mut ClockSample(&self.clock), deadline)?;
        let view = self.state.view();
        let selection = with_native_musubi_storage_v1(
            &view,
            &self.authority,
            request.commitment.archive_id(),
            pin.query().manifest_digest,
            pin.query().finalized_height,
            self.seed.provider_id(),
            now / 1_000,
            |facts| {
                facts
                    .order
                    .provider_completion(self.seed.provider_id())
                    .is_none()
                    .then_some(PublisherSourceBindingV1 {
                        provider_id: self.seed.provider_id(),
                        order_id: facts.order.order_id,
                        manifest_digest: facts.pin.digest,
                        assignment_revision: facts.order.assignment_revision,
                    })
            },
        )?;
        drop(view);
        // Genuine completed seed assignment needs no repeated byte staging. Other assigned
        // providers still use the existing governed source worker and native Complete owner.
        let Some(binding) = selection else {
            return Ok(());
        };
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
        let header_digest = header.canonical_digest()?;
        let now = authorization.check_effect_boundary(&mut ClockSample(&self.clock), deadline)?;
        let accepted = authorize_publisher_source_v1(
            &self.state.view(),
            &self.authority,
            &binding,
            now / 1_000,
        )?;
        self.storage.stage_publisher_source(
            &header,
            accepted.deadline_epoch,
            accepted.finalized_epoch,
        )?;
        let now = authorization.check_effect_boundary(&mut ClockSample(&self.clock), deadline)?;
        authorize_publisher_source_v1(&self.state.view(), &self.authority, &binding, now / 1_000)?;

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
        let maximum = header
            .chunks
            .iter()
            .map(|chunk| chunk.length as usize)
            .max()
            .unwrap_or(0);
        ensure!(
            maximum > 0 && maximum <= sorafs_car::CHUNK_STORE_MAX_CHUNK_BYTES as usize,
            "publisher chunk extent is invalid"
        );
        norito::core::reserve_decode_allocation(maximum)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(maximum)?;
        for (index, chunk) in header.chunks.iter().enumerate() {
            bytes.resize(chunk.length as usize, 0);
            payload.read_exact(&mut bytes)?;
            ensure!(
                blake3::hash(&bytes).as_bytes() == &chunk.digest,
                "publisher chunk digest differs"
            );
            let now =
                authorization.check_effect_boundary(&mut ClockSample(&self.clock), deadline)?;
            let accepted = authorize_publisher_source_v1(
                &self.state.view(),
                &self.authority,
                &binding,
                now / 1_000,
            )?;
            let upload = PublisherSourceChunkRequestV1 {
                provider_id: *binding.provider_id.as_bytes(),
                order_id: *binding.order_id.as_bytes(),
                assignment_revision: binding.assignment_revision,
                manifest_digest: *binding.manifest_digest.as_bytes(),
                header_digest,
                upload: PublisherSourceUploadV1 {
                    index: u32::try_from(index)?,
                    bytes,
                },
            };
            self.storage
                .stage_publisher_source_chunk(&upload, accepted.finalized_epoch)?;
            bytes = upload.upload.bytes;
            let now =
                authorization.check_effect_boundary(&mut ClockSample(&self.clock), deadline)?;
            authorize_publisher_source_v1(
                &self.state.view(),
                &self.authority,
                &binding,
                now / 1_000,
            )?;
        }
        let mut trailing = [0];
        ensure!(
            payload.read(&mut trailing)? == 0,
            "publisher payload has trailing bytes"
        );
        Ok(())
    }
}
