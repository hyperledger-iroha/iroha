//! Existing live Musubi projection validation over one immutable World borrow.
//!
//! Archive-local provider and location-directory scratch uses fixed V1 capacities.
//! The availability pass consumes one ordered location cursor, including all
//! retained retired rows, rather than scanning the complete location table once
//! per archive.
//! The separate attestation pass still visits every stored attestation/location.
//! Source capture separately admits complete borrowed geometry and invocation
//! work before these validators. Directory/revision and universal package scratch
//! use the original execution pool. Static rejection descriptors do not allocate.
//! TODO: retain typed original-pool custody through semantic Unicode scratch,
//! codec failures, provider instruction errors, and signature backend workspaces.

use super::*;
use crate::execution_attempt::ExecutionAttemptError;
use crate::state::deserialize::musubi_source_read::MusubiSourceReadOnly;
use iroha_allocation::AllocationBudget;

#[path = "deserialize_world_musubi_revisions.rs"]
mod revisions;
use iroha_data_model::{
    musubi::{
        MUSUBI_MAX_ARCHIVE_LOCATIONS_V1, MUSUBI_MAX_LOCATION_PROVIDERS_V1,
        MusubiProviderBundleAttestationDigestV1,
    },
    sorafs::capacity::ProviderId,
};

const MAX_PROVIDER_OCCURRENCES: usize =
    MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 * MUSUBI_MAX_LOCATION_PROVIDERS_V1;

/// Deduplicate only the protocol-bounded provider occurrences of one archive.
#[derive(Clone, Copy)]
struct CurrentProviderSet {
    values: [ProviderId; MAX_PROVIDER_OCCURRENCES],
    len: usize,
}

impl CurrentProviderSet {
    fn new() -> Self {
        Self {
            values: [ProviderId::new([0; 32]); MAX_PROVIDER_OCCURRENCES],
            len: 0,
        }
    }

    fn insert(&mut self, provider: ProviderId) -> Result<(), ProjectionRejection> {
        if self.values[..self.len].contains(&provider) {
            return Ok(());
        }
        let Some(slot) = self.values.get_mut(self.len) else {
            return Err(ProjectionRejection::new(
                ProjectionTable::ArchiveAvailability,
                "healthy provider count exceeds the V1 location capacity",
            ));
        };
        *slot = provider;
        self.len += 1;
        Ok(())
    }
}

pub(super) fn validate_musubi_live_projections(
    world: &World,
    execution_budget: &AllocationBudget,
) -> Result<(), StateRestoreError> {
    validate_musubi_live_projection_cut(&world.view(), execution_budget).map_err(|error| {
        error.map_rejection(|error| error.with_cut(ProjectionCut::Current).into_json())
    })?;
    validate_musubi_live_projection_cut(
        &world.try_block_and_revert(execution_budget)?,
        execution_budget,
    )
    .map_err(|error| {
        error.map_rejection(|error| error.with_cut(ProjectionCut::Predecessor).into_json())
    })
    .map_err(Into::into)
}

/// Verify one World cut's live Musubi availability against its SoraFS evidence.
///
/// The publication owner calls this after its last deterministic World write;
/// restore calls it separately for the current and rollback-visible cuts.
pub(in crate::state) fn validate_musubi_live_projection_cut(
    world: &impl MusubiSourceReadOnly,
    execution_budget: &AllocationBudget,
) -> Result<(), ExecutionAttemptError<ProjectionRejection>> {
    validate_musubi_live_attestation_cut(world)?;
    // Storage iterators expose canonical archive/location key order and retain
    // their cursor inline. The prior exact-source check covers every location,
    // including retired rows, so this single cursor cannot hide orphan rows.
    let mut locations = world.source_musubi_archive_locations().iter().peekable();
    for (archive_id, archive) in world.source_musubi_archives().iter() {
        archive
            .validate()
            .map_err(|error| ProjectionRejection::new(ProjectionTable::Archives, error.reason()))?;
        if archive_id != &archive.archive_id {
            return Err(ProjectionRejection::new(
                ProjectionTable::Archives,
                "archive lookup key differs from its canonical identity",
            )
            .into());
        }
        let mut active_locations = 0_usize;
        let mut healthy_providers = CurrentProviderSet::new();
        let mut maximum_location_revision = 1_u64;
        let mut current_location_count = 0_usize;
        while let Some((key, location)) =
            locations.next_if(|(key, _)| key.archive_id == *archive_id)
        {
            maximum_location_revision = maximum_location_revision.max(location.revision);
            if location.state == MusubiArchiveLocationStateV1::Retired {
                continue;
            }
            if archive
                .location_ids
                .binary_search(&key.location_id)
                .is_err()
            {
                return Err(ProjectionRejection::new(
                    ProjectionTable::ArchiveLocations,
                    "non-retired location is absent from its archive directory",
                )
                .into());
            }
            current_location_count += 1;
            let current =
                crate::smartcontracts::isi::musubi::current_location_providers(location, world);
            let current_count = current.as_ref().map_or(0, |providers| providers.len());
            let expected_state = if current_count
                >= usize::from(iroha_data_model::musubi::MUSUBI_MIN_HEALTHY_REPLICAS_V1)
            {
                MusubiArchiveLocationStateV1::Healthy
            } else {
                MusubiArchiveLocationStateV1::Degraded
            };
            if location.state != expected_state {
                return Err(ProjectionRejection::new(
                    ProjectionTable::ArchiveLocations,
                    "archive-location lifecycle state disagrees with current SoraFS evidence",
                )
                .into());
            }
            if let Some(providers) = current {
                active_locations = active_locations.checked_add(1).ok_or_else(|| {
                    ProjectionRejection::new(
                        ProjectionTable::ArchiveAvailability,
                        "active archive-location count overflows usize",
                    )
                })?;
                for provider in providers {
                    // At most four exact current locations each admit at most
                    // 64 providers. Deduplication retains only these identities.
                    healthy_providers.insert(provider)?;
                }
            }
        }
        // Canonical unique table keys and the membership checks above make
        // equal cardinality sufficient for exact directory equality. Retired
        // rows contribute to revision checks but not this current set.
        if current_location_count != archive.location_ids.len() {
            return Err(ProjectionRejection::new(
                ProjectionTable::Archives,
                "archive directory is not the exact non-retired location set",
            )
            .into());
        }
        if archive.location_revision != maximum_location_revision {
            return Err(ProjectionRejection::new(
                ProjectionTable::Archives,
                "archive location revision is not the exact maximum retained location revision",
            )
            .into());
        }
        let active_locations = u8::try_from(active_locations).map_err(|_| {
            ProjectionRejection::new(
                ProjectionTable::ArchiveAvailability,
                "active archive-location count overflows u8",
            )
        })?;
        let healthy_replicas = u16::try_from(healthy_providers.len).map_err(|_| {
            ProjectionRejection::new(
                ProjectionTable::ArchiveAvailability,
                "healthy provider count overflows u16",
            )
        })?;
        let expected_availability =
            if healthy_replicas >= iroha_data_model::musubi::MUSUBI_MIN_HEALTHY_REPLICAS_V1 {
                iroha_data_model::musubi::MusubiStorageAvailabilityV1::Selectable
            } else if active_locations > 0 && healthy_replicas > 0 {
                iroha_data_model::musubi::MusubiStorageAvailabilityV1::BelowQuorum
            } else {
                iroha_data_model::musubi::MusubiStorageAvailabilityV1::Unavailable
            };
        let projection = world
            .source_musubi_archive_availability()
            .get(archive_id)
            .ok_or_else(|| {
                ProjectionRejection::new(
                    ProjectionTable::ArchiveAvailability,
                    "archive is missing its availability projection",
                )
            })?;
        if projection.active_locations != active_locations
            || projection.healthy_replicas != healthy_replicas
            || projection.availability != expected_availability
        {
            return Err(ProjectionRejection::new(
                ProjectionTable::ArchiveAvailability,
                "availability projection is not the exact result of current SoraFS evidence",
            )
            .into());
        }
    }
    for (archive_id, projection) in world.source_musubi_archive_availability().iter() {
        if archive_id != &projection.archive_id
            || world.source_musubi_archives().get(archive_id).is_none()
            || projection.validate().is_err()
        {
            return Err(ProjectionRejection::new(
                ProjectionTable::ArchiveAvailability,
                "availability row is invalid or has no exact archive source",
            )
            .into());
        }
    }
    for (_, row) in world.source_musubi_resolver_index().iter() {
        if row.index_revision < row.selection.storage.index_revision {
            return Err(ProjectionRejection::new(
                ProjectionTable::ResolverIndex,
                "resolver row predates its embedded availability projection",
            )
            .into());
        }
    }
    revisions::validate_directory_revisions(world, execution_budget)
}

/// Keep the exact provider evidence needed by availability on the same World cut.
fn validate_musubi_live_attestation_cut(
    world: &impl MusubiSourceReadOnly,
) -> Result<(), ProjectionRejection> {
    for (key, record) in world.source_musubi_provider_bundle_attestations().iter() {
        record.validate().map_err(|error| {
            ProjectionRejection::new(ProjectionTable::ProviderBundleAttestations, error.reason())
        })?;
        let archive = world
            .source_musubi_archives()
            .get(&key.archive_id)
            .ok_or_else(|| {
                ProjectionRejection::new(
                    ProjectionTable::ProviderBundleAttestations,
                    "provider attestation references a missing archive",
                )
            })?;
        let binding = &record.attestation.payload.binding;
        let ingress = &archive.staging_receipt.payload.binding;
        if key != &record.key
            || record.registered_at_height < archive.registered_at_height
            || binding.network_id != ingress.network_id
            || binding.archive_id != archive.archive_id
            || binding.bundle_digest != archive.commitment.bundle_digest
            || binding.descriptor_digest != archive.commitment.descriptor_digest
            || binding.semantic_release_manifest_digest != ingress.semantic_release_manifest_digest
            || binding.source_tree_digest != archive.commitment.source_tree_digest
        {
            return Err(ProjectionRejection::new(
                ProjectionTable::ProviderBundleAttestations,
                "provider attestation disagrees with its key, archive, or ingress receipt",
            ));
        }
    }
    for (key, location) in world.source_musubi_archive_locations().iter() {
        location.validate().map_err(|error| {
            ProjectionRejection::new(ProjectionTable::ArchiveLocations, error.reason())
        })?;
        let archive = world
            .source_musubi_archives()
            .get(&location.archive_id)
            .ok_or_else(|| {
                ProjectionRejection::new(
                    ProjectionTable::ArchiveLocations,
                    "archive location references a missing archive",
                )
            })?;
        if key != &location.key() || location.revision > archive.location_revision {
            return Err(ProjectionRejection::new(
                ProjectionTable::ArchiveLocations,
                "archive-location key or revision is inconsistent with its archive",
            ));
        }
        // Location validation has already admitted the complete provider count.
        let mut references = [MusubiProviderBundleAttestationRefV1 {
            provider_id: ProviderId::new([0; 32]),
            digest: MusubiProviderBundleAttestationDigestV1::new([0; 32]),
        }; MUSUBI_MAX_LOCATION_PROVIDERS_V1];
        let mut verification_lock_digest = None;
        for (index, provider_id) in location.providers.iter().enumerate() {
            let attestation_key = MusubiProviderBundleAttestationKeyV1 {
                archive_id: location.archive_id,
                replication_order: location.replication_order,
                provider_id: *provider_id,
            };
            let record = world
                .source_musubi_provider_bundle_attestations()
                .get(&attestation_key)
                .ok_or_else(|| {
                    ProjectionRejection::new(
                        ProjectionTable::ArchiveLocations,
                        "archive location references a missing exact provider attestation",
                    )
                })?;
            let digest = record.attestation.payload.binding.verification_lock_digest;
            if verification_lock_digest.is_some_and(|expected| expected != digest) {
                return Err(ProjectionRejection::new(
                    ProjectionTable::ArchiveLocations,
                    "archive-location provider attestations disagree on the verification lock",
                ));
            }
            verification_lock_digest = Some(digest);
            references[index] = MusubiProviderBundleAttestationRefV1 {
                provider_id: *provider_id,
                digest: record.attestation_digest,
            };
        }
        let expected_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
            location.archive_id,
            location.replication_order,
            &references[..location.providers.len()],
        )
        .map_err(|error| {
            ProjectionRejection::new(ProjectionTable::ArchiveLocations, error.reason())
        })?;
        if location.provider_attestation_set_digest != expected_set_digest {
            return Err(ProjectionRejection::new(
                ProjectionTable::ArchiveLocations,
                "archive-location provider-attestation set digest is not exact",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_provider_set_deduplicates_and_refuses_growth_after_the_exact_v1_capacity() {
        let mut providers = CurrentProviderSet::new();
        for index in 0..MAX_PROVIDER_OCCURRENCES {
            let mut bytes = [0x61; 32];
            bytes[..8].copy_from_slice(&(index as u64).to_be_bytes());
            let provider = ProviderId::new(bytes);
            providers.insert(provider).unwrap();
            providers.insert(provider).unwrap();
            assert_eq!(providers.len, index + 1);
        }
        let original = providers;
        assert!(providers.insert(ProviderId::new([0xff; 32])).is_err());
        assert_eq!(providers.values, original.values);
        assert_eq!(providers.len, original.len);
        assert!(
            std::mem::size_of_val(&providers)
                <= MAX_PROVIDER_OCCURRENCES * std::mem::size_of::<ProviderId>()
                    + std::mem::size_of::<usize>()
        );
    }
}
