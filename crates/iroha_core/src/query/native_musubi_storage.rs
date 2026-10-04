//! Same-cut native pin and replication observation for daemon storage coordination.
//!
//! This read grants no archive-manager permission and never returns provider inventory bytes.
//! Staging writes remain separately gated by `authorize_publisher_source_v1`; registration still
//! belongs to the publisher's ordinary native instructions.

use crate::{
    query::{
        provider_admission::read_provider_admission_at_native_current_v1,
        provider_ingest_source::has_provider_completion_permission_v1,
        signer_check::{SignerCertifiedWalkV1, with_native_check_read_limits},
    },
    smartcontracts::isi::musubi::{
        current_location_providers, validate_replication_order_archive_binding,
    },
    state::{StateReadOnly, StateView, WorldReadOnly},
};
use iroha_data_model::{
    account::AccountId,
    block::consensus::SumeragiRootScope,
    musubi::{
        ArchiveId, MUSUBI_MAX_LOCATION_PROVIDERS_V1, MUSUBI_MIN_HEALTHY_REPLICAS_V1,
        MusubiArchiveLocationV1, MusubiArchiveRecordV1, MusubiReplicationOrderLocationLifecycleV1,
    },
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            ManifestDigest, PinManifestRecord, PinStatus, ReplicationOrderRecord,
            ReplicationOrderStatus, derive_sorafs_auto_replication_order_id_v1,
        },
    },
};
use mv::storage::StorageReadOnly;
use sorafs_manifest::capacity::ReplicationOrderV1;

/// Current observation refused; no local row or allocation failure becomes successful evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NativeMusubiStorageReadErrorV1 {
    /// Exact selected pin, archive, assignment, authority or lifecycle differs.
    #[error("native Musubi storage binding rejected")]
    Rejected,
    /// Original certified history or bounded read resources are unavailable.
    #[error("native Musubi storage observation unavailable")]
    Unavailable,
}

/// Borrowed facts authenticated together at the actual native current cut.
/// Fields are observations only; they do not authorize staging, signing or registry mutation.
pub struct NativeMusubiStorageObservationV1<'a> {
    /// Original archive's current record.
    pub archive: &'a MusubiArchiveRecordV1,
    /// Current pin bound to the independently selected submitter and archive.
    pub pin: &'a PinManifestRecord,
    /// Exact native automatic replication order.
    pub order: &'a ReplicationOrderRecord,
    /// Strictly sorted providers with current authenticated completion and admission.
    pub completed_providers: &'a [ProviderId],
    /// Whether native execution has completed the exact full assignment set.
    pub complete: bool,
    /// Exact current registered location, if the same order has already been consumed.
    pub location: Option<&'a MusubiArchiveLocationV1>,
}

/// Authenticate one exact automatic order and invoke a bounded borrower without cloning rows.
/// `floor_height` is independently supplied original successful pin inclusion; this reader does
/// not replace the caller's exact-envelope carrier verification.
/// # Errors
/// Refuses non-Global, uncertified/future cuts, inactive or substituted pins/orders, expired
/// admission, revoked completion authority and any retired or inconsistent location binding.
pub fn with_native_musubi_storage_v1<R>(
    view: &StateView<'_>,
    pin_authority: &AccountId,
    archive_id: ArchiveId,
    manifest: ManifestDigest,
    floor_height: u64,
    seed_provider: ProviderId,
    now_secs: u64,
    consume: impl FnOnce(NativeMusubiStorageObservationV1<'_>) -> R,
) -> Result<R, NativeMusubiStorageReadErrorV1> {
    use NativeMusubiStorageReadErrorV1::{Rejected, Unavailable};
    with_native_check_read_limits(|| {
        let height = u64::try_from(view.height()).map_err(|_| Unavailable)?;
        if now_secs == 0
            || floor_height < 2
            || height < floor_height
            || crate::sumeragi::lanes::routing::committed_root_scope(view.world())
                != Some(SumeragiRootScope::Global)
        {
            return Err(Rejected);
        }
        let chain = SignerCertifiedWalkV1::new(view).map_err(|_| Unavailable)?;
        let receipt = chain
            .walk(height, height)
            .next()
            .ok_or(Unavailable)?
            .map_err(|_| Unavailable)?;
        let certified = receipt.in_view(view).map_err(|_| Unavailable)?;
        let instance = SumeragiRootScope::Global
            .instance_id(
                &crate::sumeragi::crypto::BlsCrypto::new(),
                *view.network_id(),
                view.chain_id().as_str(),
            )
            .map_err(|_| Rejected)?;
        if certified
            .header()
            .is_none_or(|header| header.instance != instance)
        {
            return Err(Rejected);
        }
        let finalized = view
            .authenticated_query_ledger_time_ms()
            .ok_or(Unavailable)?
            / 1_000;
        let now = now_secs.max(finalized);
        let world = view.world();
        let archive = world.musubi_archives().get(&archive_id).ok_or(Rejected)?;
        archive.validate().map_err(|_| Rejected)?;
        let pin = world.pin_manifests().get(&manifest).ok_or(Rejected)?;
        let order_id = derive_sorafs_auto_replication_order_id_v1(&manifest);
        let order = world.replication_orders().get(&order_id).ok_or(Rejected)?;
        let lifecycle = validate_replication_order_archive_binding(archive, &order_id, world)
            .map_err(|_| Rejected)?;
        if pin.submitted_by != *pin_authority
            || pin.digest != manifest
            || !matches!(pin.status, PinStatus::Approved(_))
            || pin.policy.retention_epoch <= now
            || archive.staging_receipt.payload.binding.network_id != *view.network_id()
            || pin.root_cid != archive.commitment.root_cid
            || pin.chunker != archive.commitment.chunker
            || pin.chunk_digest_sha3_256 != *archive.commitment.chunk_plan_digest.as_bytes()
            || pin.por_root != *archive.commitment.por_root.as_bytes()
            || pin.content_length != archive.commitment.content_length
            || order.order_id != order_id
            || order.manifest_digest != manifest
            || order.manifest_root_cid != pin.root_cid
            || order.musubi_archive != Some(archive_id)
            || order.assignment_revision == 0
            || order.issued_epoch > finalized
            || !matches!(
                order.status,
                ReplicationOrderStatus::Pending | ReplicationOrderStatus::Completed(_)
            )
            || matches!(order.status, ReplicationOrderStatus::Pending) && order.deadline_epoch < now
            || order.provider_completions.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
        {
            return Err(Rejected);
        }
        let limit = 256 * 1024;
        let canonical: ReplicationOrderV1 = norito::decode_canonical_with_limits(
            &order.canonical_order,
            norito::DecodeLimits::new(limit, limit, limit, limit * 4, 32),
        )
        .map_err(|_| Unavailable)?;
        canonical.validate().map_err(|_| Rejected)?;
        if canonical.assignments.len() < usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1)
            || canonical.assignments.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
            || canonical.order_id != *order_id.as_bytes()
            || canonical.manifest_digest != *manifest.as_bytes()
            || canonical.manifest_cid != pin.root_cid.as_bytes()
            || canonical.chunking_profile != pin.chunker.to_handle()
            || canonical.issued_at != order.issued_epoch
            || canonical.deadline_at != order.deadline_epoch
            || !canonical
                .assignments
                .iter()
                .any(|row| row.provider_id == *seed_provider.as_bytes())
        {
            return Err(Rejected);
        }
        let mut providers = [ProviderId::new([0; 32]); MUSUBI_MAX_LOCATION_PROVIDERS_V1];
        for (slot, completed) in providers.iter_mut().zip(&order.provider_completions) {
            let provider = completed.provider_id;
            let authority = world
                .provider_ingest_completion_authorities()
                .get(&provider)
                .ok_or(Rejected)?;
            if !canonical
                .assignments
                .iter()
                .any(|row| row.provider_id == *provider.as_bytes())
                || !authority.is_valid()
                || authority != &completed.completion_authority
                || completed.completed_by != authority.completion_signer
                || completed.assignment_revision != order.assignment_revision
                || world.provider_owners().get(&provider) != Some(&authority.provider_owner)
                || !has_provider_completion_permission_v1(
                    world,
                    &authority.completion_signer,
                    provider,
                )
            {
                return Err(Rejected);
            }
            read_provider_admission_at_native_current_v1(view, &receipt, provider, now)
                .map_err(|_| Unavailable)?
                .ok_or(Rejected)?;
            *slot = provider;
        }
        let providers = &mut providers[..order.provider_completions.len()];
        providers.sort_unstable();
        if providers.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Rejected);
        }
        let complete = matches!(order.status, ReplicationOrderStatus::Completed(_));
        if complete && providers.len() != canonical.assignments.len() {
            return Err(Rejected);
        }
        let location = match lifecycle {
            MusubiReplicationOrderLocationLifecycleV1::PreLocation => None,
            MusubiReplicationOrderLocationLifecycleV1::Active(key) => {
                let location = world.musubi_archive_locations().get(key).ok_or(Rejected)?;
                if !complete
                    || location.pin_manifest != manifest
                    || location.replication_order != order_id
                    || location.expires_at_epoch <= now
                    || current_location_providers(location, world)
                        .is_none_or(|current| &*current != providers)
                {
                    return Err(Rejected);
                }
                Some(location)
            }
            MusubiReplicationOrderLocationLifecycleV1::Retired(_) => return Err(Rejected),
        };
        Ok(consume(NativeMusubiStorageObservationV1 {
            archive,
            pin,
            order,
            completed_providers: providers,
            complete,
            location,
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::isi::Log;
    #[test]
    fn actual_native_cut_never_substitutes_missing_pin_or_archive_and_inherits_resource_refusal() {
        let signer = KeyPair::from_seed(vec![0xD1; 32], Algorithm::Ed25519);
        let authority = AccountId::new(signer.public_key().clone());
        let mut config = TestChainConfig::new(World::new(), 1_000);
        config.genesis_key = signer.clone();
        let mut chain = CertifiedTestChain::start(config).unwrap();
        let called = std::cell::Cell::new(false);
        let read = |chain: &CertifiedTestChain, floor| {
            with_native_musubi_storage_v1(
                &chain.state().view(),
                &authority,
                ArchiveId::new([1; 32]),
                ManifestDigest::new([2; 32]),
                floor,
                ProviderId::new([3; 32]),
                1,
                |_| called.set(true),
            )
        };
        assert_eq!(
            read(&chain, 2),
            Err(NativeMusubiStorageReadErrorV1::Rejected)
        );
        let log = chain.sign(
            &signer,
            [Log::new(
                iroha_logger::Level::INFO,
                "native storage current cut".to_owned(),
            )
            .into()],
            chain.committed(chain.height()).block_time_ms() + 1,
        );
        assert_eq!(chain.commit(vec![log]), [true]);
        assert_eq!(
            read(&chain, 2),
            Err(NativeMusubiStorageReadErrorV1::Rejected)
        );
        assert_eq!(
            read(&chain, 3),
            Err(NativeMusubiStorageReadErrorV1::Rejected)
        );
        let zero =
            norito::DecodeLimits::new(64 * 1024 * 1024, 64 * 1024 * 1024, 64 * 1024 * 1024, 0, 128);
        assert_eq!(
            norito::with_decode_limits_scope(zero, || read(&chain, 2)),
            Err(NativeMusubiStorageReadErrorV1::Unavailable)
        );
        assert!(
            !called.get(),
            "no unavailable native cut can invoke the borrower"
        );
    }
}
