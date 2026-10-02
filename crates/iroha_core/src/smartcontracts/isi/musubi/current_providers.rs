//! Fixed local evidence for the existing current-provider predicate.
//!
//! Both completion comparison and returned provider identities use the admitted
//! V1 location bound. No heap container or full attestation clone is needed.
//! TODO: signature backends and surrounding projection traversal still require
//! their original allocation/work owner; these fixed containers do not fund them.

use super::*;
use crate::state::deserialize::musubi_source_read::MusubiSourceReadOnly;
use iroha_data_model::sorafs::capacity::ProviderId;

/// Current provider identities with protocol-bounded, allocation-free storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CurrentLocationProviders {
    values: [ProviderId; MUSUBI_MAX_LOCATION_PROVIDERS_V1],
    len: usize,
}

impl std::ops::Deref for CurrentLocationProviders {
    type Target = [ProviderId];

    fn deref(&self) -> &Self::Target {
        &self.values[..self.len]
    }
}

impl IntoIterator for CurrentLocationProviders {
    type Item = ProviderId;
    type IntoIter =
        std::iter::Take<std::array::IntoIter<ProviderId, MUSUBI_MAX_LOCATION_PROVIDERS_V1>>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter().take(self.len)
    }
}

/// Recheck complete attestation and current SoraFS authority before returning identities.
pub(crate) fn current_location_providers(
    location: &MusubiArchiveLocationV1,
    world: &impl MusubiSourceReadOnly,
) -> Option<CurrentLocationProviders> {
    if location.state == MusubiArchiveLocationStateV1::Retired || location.validate().is_err() {
        return None;
    }
    let key = location.key();
    if !world
        .source_musubi_locations_by_pin()
        .get(&location.pin_manifest)
        .is_some_and(|reference| reference.active && reference.location == key)
    {
        return None;
    }
    let archive = world.source_musubi_archives().get(&location.archive_id)?;
    archive.validate().ok()?;
    if !matches!(
        validate_replication_order_archive_binding(
            archive,
            &location.replication_order,
            world,
        ),
        Ok(MusubiReplicationOrderLocationLifecycleV1::Active(bound_location))
            if *bound_location == key
    ) {
        return None;
    }
    let pin = world.source_pin_manifests().get(&location.pin_manifest)?;
    if !pin.status.is_active()
        || pin.root_cid != archive.commitment.root_cid
        || pin.chunker != archive.commitment.chunker
        || pin.chunk_digest_sha3_256 != *archive.commitment.chunk_plan_digest.as_bytes()
        || pin.por_root != *archive.commitment.por_root.as_bytes()
        || pin.content_length != archive.commitment.content_length
        || pin.policy.retention_epoch != location.expires_at_epoch
    {
        return None;
    }
    let order = world
        .source_replication_orders()
        .get(&location.replication_order)?;
    if order.manifest_digest != location.pin_manifest
        || order.manifest_root_cid != archive.commitment.root_cid
        || !matches!(order.status, ReplicationOrderStatus::Completed(_))
    {
        return None;
    }
    // Reject an oversized/mismatched completion collection before reading it.
    // The admitted location has 1..=64 providers, so sorting this fixed prefix
    // never requests scratch from the allocator.
    if order.provider_completions.len() != location.providers.len() {
        return None;
    }
    let mut completions = [None; MUSUBI_MAX_LOCATION_PROVIDERS_V1];
    for (slot, completion) in completions.iter_mut().zip(&order.provider_completions) {
        *slot = Some(completion);
    }
    let completions = &mut completions[..location.providers.len()];
    completions.sort_unstable_by_key(|completion| completion.map(|row| row.provider_id));
    if !completions
        .iter()
        .zip(&location.providers)
        .all(|(completion, provider)| completion.is_some_and(|row| row.provider_id == *provider))
    {
        return None;
    }
    let records = load_location_provider_attestations(archive, location, world).ok()?;
    let mut providers = CurrentLocationProviders {
        values: [ProviderId::new([0; 32]); MUSUBI_MAX_LOCATION_PROVIDERS_V1],
        len: 0,
    };
    for ((provider, record), completion) in location
        .providers
        .iter()
        .zip(records.iter())
        .zip(completions.iter())
    {
        let reverse_key = MusubiProviderLocationKeyV1::new(*provider, key);
        if world
            .source_musubi_locations_by_provider()
            .get(&reverse_key)
            .is_none()
        {
            continue;
        }
        let Some(owner) = world.source_provider_owners().get(provider) else {
            continue;
        };
        // The exact sorted prefix was fully initialized and checked above.
        let completion = completion.as_ref()?;
        let binding = &record.attestation.payload.binding;
        if completion.completed_by == *owner
            && completion.completion_authority.provider_owner == *owner
            && binding.provider_id == *provider
            && binding.completed_by == completion.completed_by
            && binding.completion_authority == completion.completion_authority
            && binding.assignment_revision == completion.assignment_revision
            && binding.completion_epoch == completion.completion_epoch
            && binding.finalized_anchor == completion.finalized_anchor
        {
            providers.values[providers.len] = *provider;
            providers.len += 1;
        }
    }
    Some(providers)
}
