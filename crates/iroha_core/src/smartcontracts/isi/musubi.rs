//! Musubi V1 package-registry instruction and query handlers.
//!
//! The registry uses only typed first-release stores. No legacy state-path
//! decoding or compatibility aliases live here.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;
use iroha_data_model::{
    asset::AssetId,
    domain::DomainId,
    events::data::{DataEvent, musubi::prelude::*},
    governance::types::ProposalKind,
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError},
        musubi::*,
    },
    musubi::*,
    query::{error::QueryExecutionFail, musubi::prelude::*},
    sorafs::pin_registry::ReplicationOrderStatus,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use norito::codec::Encode;

use super::prelude::*;
use crate::{
    prelude::ValidSingularQuery,
    smartcontracts::Execute,
    state::{
        GovernanceProposalStatus, MusubiResolverIndexRevisionV1, StateReadOnly, StateTransaction,
        WorldReadOnly,
    },
    telemetry::{MusubiGovernanceActionV1, MusubiGovernanceRejectionReasonV1},
};

impl Execute for RegisterMusubiNamespaceBindingV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::NamespaceBinding,
            |state_transaction, rejection_reason| {
                self.binding
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                if let Some(existing) = state_transaction
                    .world
                    .musubi_namespace_bindings
                    .get(&self.binding.namespace)
                {
                    if existing != &self.binding {
                        return Err(invariant(format!(
                            "Musubi namespace '{}' is already bound",
                            self.binding.namespace
                        )));
                    }
                    ensure_namespace_current_owner(
                        existing,
                        authority,
                        state_transaction,
                        rejection_reason,
                    )?;
                    return Ok(());
                }
                let policy = state_transaction.world.musubi_registry_policy.get().clone();
                classify_governance_rejection(
                    ensure_policy_revision(&policy, self.expected_policy_revision),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                ensure_admitted(
                    &policy,
                    Some(self.binding.home_dataspace),
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                ensure_namespace_registration_owner(
                    &self.binding,
                    authority,
                    state_transaction,
                    rejection_reason,
                )?;
                let event = MusubiEvent::NamespaceBound(self.binding.clone());
                state_transaction
                    .world
                    .musubi_namespace_bindings
                    .insert(self.binding.namespace.clone(), self.binding);
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for RegisterMusubiArchiveV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::ArchiveRegistration,
            |state_transaction, rejection_reason| {
                self.commitment
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                self.staging_receipt
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let archive_id = self.commitment.archive_id();
                if let Some(existing) = state_transaction.world.musubi_archives.get(&archive_id) {
                    if existing.commitment != self.commitment
                        || existing.staging_receipt != self.staging_receipt
                    {
                        return Err(invariant(format!(
                            "Musubi archive '{}' is already registered with a different commitment or staging receipt",
                            digest_label(archive_id.as_bytes())
                        )));
                    }
                    if existing.registered_by != *authority {
                        return reject_governance_mutation(
                            rejection_reason,
                            MusubiGovernanceRejectionReasonV1::Unauthorized,
                            invariant(
                                "only the original Musubi archive registrant may replay registration",
                            ),
                        );
                    }
                    return Ok(());
                }
                let policy = state_transaction.world.musubi_registry_policy.get().clone();
                classify_governance_rejection(
                    ensure_policy_revision(&policy, self.expected_policy_revision),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                ensure_admitted(
                    &policy,
                    None,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                validate_seed_ingress_receipt(
                    &self.commitment,
                    &self.staging_receipt,
                    authority,
                    state_transaction,
                )?;
                let height = execution_height(state_transaction);
                let index_revision = state_transaction
                    .world
                    .musubi_resolver_index_revision
                    .get()
                    .get();
                state_transaction.world.musubi_archives.insert(
                    archive_id,
                    MusubiArchiveRecordV1 {
                        archive_id,
                        commitment: self.commitment,
                        staging_receipt: self.staging_receipt,
                        registered_by: authority.clone(),
                        registered_at_height: height,
                        location_revision: 1,
                        location_ids: Vec::new(),
                    },
                );
                let availability = MusubiArchiveAvailabilityV1 {
                    archive_id,
                    availability: MusubiStorageAvailabilityV1::Unavailable,
                    healthy_replicas: 0,
                    active_locations: 0,
                    finalized_height: height,
                    finalized_block_hash: execution_hash(state_transaction),
                    index_revision,
                };
                state_transaction
                    .world
                    .musubi_archive_availability
                    .insert(archive_id, availability);
                state_transaction
                    .world
                    .musubi_archive_reverse_references
                    .insert(
                        archive_id,
                        MusubiArchiveReverseReferencesV1 {
                            archive_id,
                            releases: Vec::new(),
                        },
                    );
                emit_musubi_event(
                    MusubiEvent::ArchiveRegistered(MusubiArchiveRegisteredEventV1 {
                        archive_id,
                        registered_by: authority.clone(),
                        location_revision: 1,
                        finalized_height: height,
                    }),
                    state_transaction,
                );
                emit_musubi_event(
                    MusubiEvent::ArchiveAvailabilityChanged(availability),
                    state_transaction,
                );
                Ok(())
            },
        )
    }
}

impl Execute for RegisterMusubiProviderBundleAttestationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::ArchiveLocation,
            |state_transaction, rejection_reason| {
                self.validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let key = self.attestation.key();
                let archive = state_transaction
                    .world
                    .musubi_archives
                    .get(&key.archive_id)
                    .cloned()
                    .ok_or_else(|| archive_not_found(key.archive_id))?;
                archive
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                if archive.archive_id != key.archive_id {
                    return Err(invariant(
                        "Musubi provider attestation archive has the wrong embedded identity",
                    ));
                }
                ensure_archive_manager(
                    &archive,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                if let Some(existing) = state_transaction
                    .world
                    .musubi_provider_bundle_attestations
                    .get(&key)
                {
                    existing
                        .validate()
                        .map_err(|error| invariant(error.reason()))?;
                    existing
                        .attestation
                        .verify(&existing.attestation.payload.binding)
                        .map_err(|error| invariant(error.reason()))?;
                    if existing.attestation_digest == self.attestation.digest()
                        && existing.attestation == self.attestation
                    {
                        return Ok(());
                    }
                    return Err(invariant(
                        "Musubi provider bundle attestation identity is permanently bound to different evidence",
                    ));
                }
                classify_governance_rejection(
                    ensure_revision(
                        "archive location",
                        archive.location_revision,
                        self.expected_location_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                validate_provider_bundle_attestation(
                    &archive,
                    &self.attestation,
                    state_transaction,
                )?;
                let finalized_height = execution_height(state_transaction);
                let record = MusubiProviderBundleAttestationRecordV1 {
                    key,
                    attestation_digest: self.attestation.digest(),
                    attestation: self.attestation,
                    registered_by: authority.clone(),
                    registered_at_height: finalized_height,
                };
                record
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                state_transaction
                    .world
                    .musubi_provider_bundle_attestations
                    .insert(key, record.clone());
                emit_musubi_event(
                    MusubiEvent::ProviderBundleAttestationRegistered(
                        MusubiProviderBundleAttestationRegisteredEventV1 {
                            key,
                            attestation_digest: record.attestation_digest,
                            registered_by: authority.clone(),
                            finalized_height,
                        },
                    ),
                    state_transaction,
                );
                Ok(())
            },
        )
    }
}

impl Execute for AddMusubiArchiveLocationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::ArchiveLocation,
            |state_transaction, rejection_reason| {
                self.validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let mut archive = state_transaction
                    .world
                    .musubi_archives
                    .get(&self.archive_id)
                    .cloned()
                    .ok_or_else(|| archive_not_found(self.archive_id))?;
                archive
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                if archive.archive_id != self.archive_id {
                    return Err(invariant(
                        "Musubi archive location request resolved the wrong archive record",
                    ));
                }
                ensure_archive_manager(
                    &archive,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                let key = MusubiArchiveLocationKeyV1::new(self.archive_id, self.location_id);
                let existing_location = state_transaction
                    .world
                    .musubi_archive_locations
                    .get(&key)
                    .cloned();
                match existing_location.as_ref() {
                    Some(existing) if existing.state == MusubiArchiveLocationStateV1::Retired => {
                        return Err(invariant(
                            "Musubi retired archive location identities cannot be reused",
                        ));
                    }
                    Some(existing) => {
                        if existing.key() != key {
                            return Err(invariant(
                                "Musubi archive location record has the wrong embedded identity",
                            ));
                        }
                        if archive
                            .location_ids
                            .binary_search(&self.location_id)
                            .is_err()
                        {
                            return Err(invariant(
                                "Musubi archive location directory is inconsistent",
                            ));
                        }
                        if archive.location_ids.len() > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 {
                            return Err(invariant("Musubi archive location bound is exhausted"));
                        }
                        // A committed response may be lost. Match only the fields supplied by
                        // this instruction so later lifecycle state/revision changes do not turn
                        // an exact retry into a renewal. The no-op path still validates every
                        // active reverse projection and immutable provider evidence before it
                        // returns without consuming another CAS revision or emitting an event.
                        if existing.pin_manifest == self.pin_manifest
                            && existing.replication_order == self.replication_order
                            && existing.provider_attestation_set_digest
                                == self.provider_attestation_set_digest
                            && existing.renew_after_epoch == self.renew_after_epoch
                            && existing.expires_at_epoch == self.expires_at_epoch
                        {
                            validate_exact_archive_location_replay(
                                &archive,
                                existing,
                                state_transaction,
                            )?;
                            return Ok(());
                        }
                    }
                    None => {}
                }
                classify_governance_rejection(
                    ensure_revision(
                        "archive location",
                        archive.location_revision,
                        self.expected_location_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                if existing_location.is_none() {
                    if archive.location_ids.len() >= MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 {
                        return Err(invariant("Musubi archive location bound is exhausted"));
                    }
                    archive.location_ids.push(self.location_id);
                    archive.location_ids.sort();
                    archive.location_ids.dedup();
                }
                if archive.location_ids.len() > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 {
                    return Err(invariant("Musubi archive location bound is exhausted"));
                }
                let providers = validate_sorafs_location(
                    &archive,
                    &self.pin_manifest,
                    &self.replication_order,
                    self.provider_attestation_set_digest,
                    self.expires_at_epoch,
                    state_transaction,
                )?;
                let next_revision = next_revision(archive.location_revision, "archive location")?;
                let state = if providers.len() >= usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1) {
                    MusubiArchiveLocationStateV1::Healthy
                } else {
                    MusubiArchiveLocationStateV1::Degraded
                };
                let location = MusubiArchiveLocationV1 {
                    location_id: self.location_id,
                    archive_id: self.archive_id,
                    pin_manifest: self.pin_manifest,
                    replication_order: self.replication_order,
                    providers,
                    provider_attestation_set_digest: self.provider_attestation_set_digest,
                    renew_after_epoch: self.renew_after_epoch,
                    expires_at_epoch: self.expires_at_epoch,
                    finalized_height: execution_height(state_transaction),
                    revision: next_revision,
                    state,
                };
                location
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let provider_count = u8::try_from(location.providers.len()).map_err(|_| {
                    invariant("Musubi archive location provider count overflows u8")
                })?;
                let location_event = MusubiArchiveLocationEventV1 {
                    location: key,
                    pin_manifest: location.pin_manifest,
                    replication_order: location.replication_order,
                    provider_attestation_set_digest: location.provider_attestation_set_digest,
                    provider_count,
                    transition: if existing_location.is_some() {
                        MusubiArchiveLocationTransitionV1::Renewed
                    } else {
                        MusubiArchiveLocationTransitionV1::Added
                    },
                    state: location.state,
                    revision: location.revision,
                    finalized_height: location.finalized_height,
                };
                bind_location_reverse_indices(
                    existing_location.as_ref(),
                    &location,
                    state_transaction,
                )?;
                archive.location_revision = next_revision;
                archive
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                state_transaction
                    .world
                    .musubi_archive_locations
                    .insert(key, location);
                state_transaction
                    .world
                    .musubi_archives
                    .insert(self.archive_id, archive);
                emit_musubi_event(
                    MusubiEvent::ArchiveLocationChanged(location_event),
                    state_transaction,
                );
                refresh_musubi_locations(&[key], state_transaction)?;
                Ok(())
            },
        )
    }
}

impl Execute for RetireMusubiArchiveLocationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::ArchiveLocation,
            |state_transaction, rejection_reason| {
                let mut archive = state_transaction
                    .world
                    .musubi_archives
                    .get(&self.archive_id)
                    .cloned()
                    .ok_or_else(|| archive_not_found(self.archive_id))?;
                ensure_archive_manager(
                    &archive,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                classify_governance_rejection(
                    ensure_revision(
                        "archive location",
                        archive.location_revision,
                        self.expected_location_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                let key = MusubiArchiveLocationKeyV1::new(self.archive_id, self.location_id);
                let mut location = state_transaction
                    .world
                    .musubi_archive_locations
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi archive location was not found"))?;
                if location.state == MusubiArchiveLocationStateV1::Retired {
                    return Err(invariant("Musubi archive location is already retired"));
                }
                ensure_locations_may_be_invalidated(&[key], state_transaction.world())?;
                self.reason
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let next_revision = next_revision(archive.location_revision, "archive location")?;
                location.state = MusubiArchiveLocationStateV1::Retired;
                location.finalized_height = execution_height(state_transaction);
                location.revision = next_revision;
                retire_location_reverse_indices(&location, state_transaction)?;
                let location_event = MusubiArchiveLocationEventV1 {
                    location: key,
                    pin_manifest: location.pin_manifest,
                    replication_order: location.replication_order,
                    provider_attestation_set_digest: location.provider_attestation_set_digest,
                    provider_count: u8::try_from(location.providers.len()).map_err(|_| {
                        invariant("Musubi archive location provider count overflows u8")
                    })?,
                    transition: MusubiArchiveLocationTransitionV1::Retired,
                    state: location.state,
                    revision: location.revision,
                    finalized_height: location.finalized_height,
                };
                archive.location_revision = next_revision;
                archive
                    .location_ids
                    .retain(|location_id| *location_id != self.location_id);
                archive
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                state_transaction
                    .world
                    .musubi_archive_locations
                    .insert(key, location);
                state_transaction
                    .world
                    .musubi_archives
                    .insert(self.archive_id, archive);
                emit_musubi_event(
                    MusubiEvent::ArchiveLocationChanged(location_event),
                    state_transaction,
                );
                refresh_archive_availability(self.archive_id, state_transaction)?;
                Ok(())
            },
        )
    }
}

impl Execute for PublishMusubiReleaseV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Publish,
            |state_transaction, rejection_reason| {
                self.namespace
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                self.publication
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let release_id = self.publication.manifest.release.clone();
                let release_digest = self.publication.manifest.release_digest();
                if let Some(existing) = state_transaction.world.musubi_releases.get(&release_id) {
                    if existing.release_digest != release_digest
                        || existing.manifest != self.publication.manifest
                        || !state_transaction
                            .world
                            .musubi_packages
                            .get(&release_id.package)
                            .is_some_and(|package| package.claimed_namespace == self.namespace)
                    {
                        return Err(invariant(format!(
                            "Musubi release '{release_id}' is permanently bound to different commitments"
                        )));
                    }
                    if existing.published_by != *authority {
                        return reject_governance_mutation(
                            rejection_reason,
                            MusubiGovernanceRejectionReasonV1::Unauthorized,
                            invariant(
                                "only the original Musubi release publisher may replay publication",
                            ),
                        );
                    }
                    return Ok(());
                }
                let policy = state_transaction.world.musubi_registry_policy.get().clone();
                classify_governance_rejection(
                    ensure_policy_revision(&policy, self.expected_policy_revision),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                ensure_admitted(
                    &policy,
                    Some(release_id.package.home_dataspace),
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                validate_publication_snapshot(&self.publication, state_transaction)?;
                validate_resolution_proof(&self.publication, state_transaction.world())?;
                let archive = state_transaction
                    .world
                    .musubi_archives
                    .get(&self.publication.manifest.archive_id)
                    .cloned()
                    .ok_or_else(|| archive_not_found(self.publication.manifest.archive_id))?;
                let availability = state_transaction
                    .world
                    .musubi_archive_availability
                    .get(&archive.archive_id)
                    .copied()
                    .ok_or_else(|| invariant("Musubi archive has no availability projection"))?;
                if availability.availability != MusubiStorageAvailabilityV1::Selectable {
                    return Err(invariant(
                        "Musubi release archive has not reached finalized replication quorum",
                    ));
                }
                validate_publication_archive_evidence(
                    &self.publication,
                    &archive,
                    authority,
                    state_transaction.world(),
                )?;

                let height = execution_height(state_transaction);
                let existing_package = state_transaction
                    .world
                    .musubi_packages
                    .get(&release_id.package)
                    .cloned();
                let first_claim = existing_package.is_none();
                let (package, governance_advance, first_member, first_metadata) =
                    match existing_package {
                        Some(mut package) => {
                            if package.claimed_namespace != self.namespace {
                                return Err(invariant(
                                    "Musubi publication namespace does not match the package claim",
                                ));
                            }
                            let expected = self.expected_governance_revision.ok_or_else(|| {
                        invalid_parameter(
                            "existing Musubi package publication requires a governance revision",
                        )
                    })?;
                            classify_governance_rejection(
                                ensure_revision(
                                    "package governance",
                                    package.revisions.governance,
                                    expected,
                                ),
                                rejection_reason,
                                MusubiGovernanceRejectionReasonV1::StaleRevision,
                            )?;
                            ensure_package_capability(
                                &release_id.package,
                                authority,
                                PackageCapability::Publish,
                                state_transaction.world(),
                                rejection_reason,
                            )?;
                            let advance = plan_package_governance_advance(
                                &mut package,
                                height,
                                None,
                                state_transaction.world(),
                            )?;
                            (package, Some(advance), None, None)
                        }
                        None => {
                            if self.expected_governance_revision.is_some() {
                                return Err(invalid_parameter(
                                    "first Musubi package claim must not supply a governance revision",
                                ));
                            }
                            let binding = namespace_binding_for_package(
                                &self.namespace,
                                &release_id.package,
                                state_transaction.world(),
                            )?;
                            ensure_namespace_claim_authority(
                                &binding,
                                self.namespace_delegation.as_ref(),
                                authority,
                                state_transaction,
                                rejection_reason,
                            )?;
                            let package = MusubiPackageRecordV1 {
                                package: release_id.package.clone(),
                                claimed_namespace: self.namespace,
                                claimed_namespace_binding: binding.digest(),
                                owners: vec![authority.clone()],
                                member_accounts: vec![authority.clone()],
                                claimed_at_height: height,
                                revisions: MusubiPackageRevisionsV1 {
                                    governance: 1,
                                    metadata: 1,
                                    archive_locations: 1,
                                },
                            };
                            let member = MusubiPackageMemberV1 {
                                package: release_id.package.clone(),
                                account: authority.clone(),
                                role: MusubiPackageRoleV1::Owner,
                                accepted_at_height: height,
                                governance_revision: 1,
                            };
                            member
                                .validate()
                                .map_err(|error| invariant(error.reason()))?;
                            let metadata = MusubiPackageMetadataRecordV1 {
                                package: release_id.package.clone(),
                                metadata: self.publication.manifest.metadata.clone(),
                                revision: 1,
                                changed_by: authority.clone(),
                                changed_at_height: height,
                            };
                            metadata
                                .validate()
                                .map_err(|error| invariant(error.reason()))?;
                            (package, None, Some(member), Some(metadata))
                        }
                    };
                package
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;

                let initial_reason = MusubiReasonV1::new("initial publication")
                    .expect("static Musubi reason is valid");
                let yank = MusubiReleaseYankV1 {
                    release: release_id.clone(),
                    yanked: false,
                    reason: initial_reason,
                    changed_by: authority.clone(),
                    changed_at_height: height,
                    revision: 1,
                };
                let record = MusubiReleaseRecordV1 {
                    manifest: self.publication.manifest.clone(),
                    release_digest,
                    published_by: authority.clone(),
                    published_at_height: height,
                    yank: yank.clone(),
                    artifact_governance: MusubiArtifactGovernanceStateV1::Available,
                    revisions: MusubiReleaseRevisionsV1 {
                        yank: 1,
                        artifact_governance: 1,
                    },
                };
                record
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;

                let planned_index_revision = plan_resolver_index_revision(state_transaction)?;
                let index_revision = planned_index_revision.get();
                let reverse_references = plan_archive_reverse_reference(
                    archive.archive_id,
                    release_id.clone(),
                    state_transaction.world(),
                )?;
                let latest_selectable = state_transaction
                    .world
                    .musubi_resolver_index
                    .range(package_release_start(&release_id.package)..)
                    .take_while(|(release, _)| release.package == release_id.package)
                    .filter(|(_, row)| row.selection.fresh_selectable())
                    .map(|(release, _)| release.version.clone())
                    .chain(std::iter::once(release_id.version.clone()))
                    .max();
                let directory =
                    plan_package_directory_entry(&package, latest_selectable, index_revision)?;
                let row = MusubiResolverReleaseRowV1 {
                    release: release_id.clone(),
                    release_digest,
                    archive_id: archive.archive_id,
                    source_digest: archive.commitment.source_tree_digest,
                    interface_digest: self.publication.manifest.interface_digest,
                    abi: self.publication.manifest.abi,
                    dependencies: self.publication.manifest.dependencies,
                    selection: MusubiReleaseSelectionStateV1 {
                        yank,
                        storage: availability,
                        governance: MusubiArtifactGovernanceStateV1::Available,
                    },
                    index_revision,
                };
                row.validate().map_err(|error| invariant(error.reason()))?;
                let package_claimed_event = first_claim.then(|| {
                    MusubiEvent::PackageClaimed(MusubiPackageClaimedEventV1 {
                        package: release_id.package.clone(),
                        namespace: package.claimed_namespace.clone(),
                        claimed_by: authority.clone(),
                        governance_revision: package.revisions.governance,
                        finalized_height: height,
                    })
                });
                let metadata_event = first_metadata
                    .as_ref()
                    .cloned()
                    .map(MusubiEvent::PackageMetadataChanged);

                *state_transaction
                    .world
                    .musubi_resolver_index_revision
                    .get_mut() = planned_index_revision;
                if let Some(member) = first_member {
                    state_transaction
                        .world
                        .musubi_package_members
                        .insert(member.key(), member.clone());
                    upsert_maintainer_directory(
                        MusubiMaintainerDirectoryEntryV1::Accepted(member),
                        state_transaction,
                    );
                }
                if let Some(metadata) = first_metadata {
                    state_transaction
                        .world
                        .musubi_package_metadata
                        .insert(metadata.package.clone(), metadata);
                }
                state_transaction
                    .world
                    .musubi_packages
                    .insert(release_id.package.clone(), package);
                state_transaction
                    .world
                    .musubi_releases
                    .insert(release_id.clone(), record);
                state_transaction
                    .world
                    .musubi_archive_reverse_references
                    .insert(archive.archive_id, reverse_references);
                state_transaction
                    .world
                    .musubi_resolver_index
                    .insert(release_id.clone(), row);
                state_transaction
                    .world
                    .musubi_public_directory
                    .insert(directory.selector.clone(), directory);
                if let Some(event) = package_claimed_event {
                    emit_musubi_event(event, state_transaction);
                }
                if let Some(event) = metadata_event {
                    emit_musubi_event(event, state_transaction);
                }
                if let Some(advance) = governance_advance {
                    advance.apply_invitation_updates(state_transaction);
                    for event in advance.invitation_events {
                        emit_musubi_event(event, state_transaction);
                    }
                }
                emit_musubi_event(
                    MusubiEvent::ReleasePublished(MusubiReleasePublishedEventV1 {
                        release: release_id,
                        release_digest,
                        archive_id: archive.archive_id,
                        published_by: authority.clone(),
                        finalized_height: height,
                    }),
                    state_transaction,
                );
                Ok(())
            },
        )
    }
}

impl Execute for SetMusubiReleaseYankV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Yank,
            |state_transaction, rejection_reason| {
                self.release
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                self.reason
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let mut release = state_transaction
                    .world
                    .musubi_releases
                    .get(&self.release)
                    .cloned()
                    .ok_or_else(|| release_not_found(&self.release))?;
                ensure_package_capability(
                    &self.release.package,
                    authority,
                    PackageCapability::Yank,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                classify_governance_rejection(
                    ensure_revision(
                        "release yank",
                        release.revisions.yank,
                        self.expected_yank_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                if release.yank.yanked == self.yanked {
                    return Err(invariant("Musubi release yank state is unchanged"));
                }
                let next = next_revision(release.revisions.yank, "release yank")?;
                release.yank = MusubiReleaseYankV1 {
                    release: self.release.clone(),
                    yanked: self.yanked,
                    reason: self.reason,
                    changed_by: authority.clone(),
                    changed_at_height: execution_height(state_transaction),
                    revision: next,
                };
                release.revisions.yank = next;
                let index_revision = bump_resolver_index_revision(state_transaction)?;
                let mut row = state_transaction
                    .world
                    .musubi_resolver_index
                    .get(&self.release)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi release is absent from the resolver index"))?;
                row.selection.yank = release.yank.clone();
                row.index_revision = index_revision;
                release
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                row.validate().map_err(|error| invariant(error.reason()))?;
                let event = MusubiEvent::ReleaseYankChanged(MusubiReleaseYankEventV1 {
                    yank: release.yank.clone(),
                    archive_id: release.manifest.archive_id,
                });
                state_transaction
                    .world
                    .musubi_releases
                    .insert(self.release.clone(), release);
                state_transaction
                    .world
                    .musubi_resolver_index
                    .insert(self.release.clone(), row);
                refresh_directory_for_package(
                    &self.release.package,
                    index_revision,
                    state_transaction,
                )?;
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for SetMusubiPackageMetadataV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Metadata,
            |state_transaction, rejection_reason| {
                self.metadata
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let mut package = state_transaction
                    .world
                    .musubi_packages
                    .get(&self.package)
                    .cloned()
                    .ok_or_else(|| package_not_found(&self.package))?;
                ensure_package_capability(
                    &self.package,
                    authority,
                    PackageCapability::Metadata,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                classify_governance_rejection(
                    ensure_revision(
                        "package metadata",
                        package.revisions.metadata,
                        self.expected_metadata_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                let next = next_revision(package.revisions.metadata, "package metadata")?;
                package.revisions.metadata = next;
                state_transaction
                    .world
                    .musubi_packages
                    .insert(self.package.clone(), package);
                let metadata = MusubiPackageMetadataRecordV1 {
                    package: self.package.clone(),
                    metadata: self.metadata,
                    revision: next,
                    changed_by: authority.clone(),
                    changed_at_height: execution_height(state_transaction),
                };
                state_transaction
                    .world
                    .musubi_package_metadata
                    .insert(self.package.clone(), metadata.clone());
                let index_revision = bump_resolver_index_revision(state_transaction)?;
                refresh_directory_for_package(&self.package, index_revision, state_transaction)?;
                emit_musubi_event(
                    MusubiEvent::PackageMetadataChanged(metadata),
                    state_transaction,
                );
                Ok(())
            },
        )
    }
}

impl Execute for InviteMusubiPackageMaintainerV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Invite,
            |state_transaction, rejection_reason| {
                validate_role(self.role)?;
                ensure_package_owner(
                    &self.package,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                let mut package = state_transaction
                    .world
                    .musubi_packages
                    .get(&self.package)
                    .cloned()
                    .ok_or_else(|| package_not_found(&self.package))?;
                classify_governance_rejection(
                    ensure_revision(
                        "package governance",
                        package.revisions.governance,
                        self.expected_governance_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                let height = execution_height(state_transaction);
                if self.expires_at_height <= height {
                    return Err(invalid_parameter(
                        "Musubi package invitation expiry must be in the future",
                    ));
                }
                if package
                    .member_accounts
                    .binary_search(&self.invited_account)
                    .is_ok()
                {
                    return Err(invariant(
                        "Musubi package invitation targets an existing member",
                    ));
                }
                if state_transaction
                    .world
                    .musubi_package_invitations
                    .get(&self.invite_id)
                    .is_some()
                {
                    return Err(invariant("Musubi package invitation id is already used"));
                }
                let active_pending = active_pending_invitation_count(
                    &self.package,
                    height,
                    state_transaction.world(),
                );
                if active_pending >= MUSUBI_MAX_PENDING_INVITATIONS_V1 {
                    return Err(invariant(
                        "Musubi package pending-invitation bound is exhausted",
                    ));
                }
                let next = next_revision(package.revisions.governance, "package governance")?;
                let invitation = MusubiMaintainerInvitationV1 {
                    invite_id: self.invite_id,
                    package: self.package.clone(),
                    invited_by: authority.clone(),
                    invited_account: self.invited_account,
                    role: self.role,
                    expected_governance_revision: next,
                    expires_at_height: self.expires_at_height,
                    state: MusubiInvitationStateV1::Pending,
                };
                invitation
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let advance = plan_package_governance_advance(
                    &mut package,
                    height,
                    None,
                    state_transaction.world(),
                )?;
                let event = MusubiEvent::MaintainerInvited(invitation.clone());
                state_transaction
                    .world
                    .musubi_packages
                    .insert(self.package, package);
                advance.apply_invitation_updates(state_transaction);
                state_transaction
                    .world
                    .musubi_package_invitations
                    .insert(self.invite_id, invitation.clone());
                upsert_maintainer_directory(
                    MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation),
                    state_transaction,
                );
                for event in advance.invitation_events {
                    emit_musubi_event(event, state_transaction);
                }
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for AcceptMusubiPackageMaintainerV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Accept,
            |state_transaction, rejection_reason| {
                let invitation = state_transaction
                    .world
                    .musubi_package_invitations
                    .get(&self.invite_id)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi package invitation was not found"))?;
                if invitation.package != self.package
                    || invitation.invited_account != *authority
                    || invitation.state != MusubiInvitationStateV1::Pending
                {
                    if invitation.invited_account != *authority {
                        *rejection_reason = MusubiGovernanceRejectionReasonV1::Unauthorized;
                    }
                    return Err(invariant(
                        "Musubi package invitation is not pending for this authority",
                    ));
                }
                if execution_height(state_transaction) > invitation.expires_at_height {
                    return Err(invariant(
                        "Musubi package invitation has expired; the next successful package governance mutation commits bounded expiry cleanup",
                    ));
                }
                let mut package = state_transaction
                    .world
                    .musubi_packages
                    .get(&self.package)
                    .cloned()
                    .ok_or_else(|| package_not_found(&self.package))?;
                classify_governance_rejection(
                    ensure_revision(
                        "package governance",
                        package.revisions.governance,
                        self.expected_governance_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                classify_governance_rejection(
                    ensure_revision(
                        "package invitation",
                        invitation.expected_governance_revision,
                        self.expected_governance_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                if package.member_accounts.binary_search(authority).is_ok() {
                    return Err(invariant(
                        "Musubi package invitation targets an existing member",
                    ));
                }
                if package.member_accounts.len() >= MUSUBI_MAX_PACKAGE_MEMBERS_V1 {
                    return Err(invariant("Musubi package member bound is exhausted"));
                }
                let maintainer_count = package
                    .member_accounts
                    .len()
                    .saturating_sub(package.owners.len());
                if matches!(invitation.role, MusubiPackageRoleV1::Maintainer(_))
                    && maintainer_count >= MUSUBI_MAX_PACKAGE_MAINTAINERS_V1
                {
                    return Err(invariant("Musubi package maintainer bound is exhausted"));
                }
                if invitation.role == MusubiPackageRoleV1::Owner
                    && package.owners.len() >= MUSUBI_MAX_PACKAGE_OWNERS_V1
                {
                    return Err(invariant("Musubi package owner bound is exhausted"));
                }
                let height = execution_height(state_transaction);
                if invitation.role == MusubiPackageRoleV1::Owner {
                    package.owners.push(authority.clone());
                    package.owners.sort();
                    package.owners.dedup();
                }
                package.member_accounts.push(authority.clone());
                package.member_accounts.sort();
                package.member_accounts.dedup();
                package
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                let advance = plan_package_governance_advance(
                    &mut package,
                    height,
                    Some((self.invite_id, MusubiInvitationStateV1::Accepted)),
                    state_transaction.world(),
                )?;
                let next = advance.revision;
                let invitation = advance
                    .terminal_invitation
                    .as_ref()
                    .cloned()
                    .ok_or_else(|| invariant("Musubi accepted invitation was not terminalized"))?;
                let member = MusubiPackageMemberV1 {
                    package: self.package.clone(),
                    account: authority.clone(),
                    role: invitation.role,
                    accepted_at_height: height,
                    governance_revision: next,
                };
                state_transaction.world.musubi_package_members.insert(
                    MusubiPackageMemberKeyV1::new(self.package.clone(), authority.clone()),
                    member.clone(),
                );
                state_transaction
                    .world
                    .musubi_packages
                    .insert(self.package, package);
                advance.apply_invitation_updates(state_transaction);
                upsert_maintainer_directory(
                    MusubiMaintainerDirectoryEntryV1::Accepted(member.clone()),
                    state_transaction,
                );
                for event in advance.invitation_events {
                    emit_musubi_event(event, state_transaction);
                }
                emit_musubi_event(MusubiEvent::MaintainerAccepted(member), state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for RevokeMusubiPackageMaintainerInvitationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Invite,
            |state_transaction, rejection_reason| {
                ensure_package_owner(
                    &self.package,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                let mut package = state_transaction
                    .world
                    .musubi_packages
                    .get(&self.package)
                    .cloned()
                    .ok_or_else(|| package_not_found(&self.package))?;
                classify_governance_rejection(
                    ensure_revision(
                        "package governance",
                        package.revisions.governance,
                        self.expected_governance_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                let invitation = state_transaction
                    .world
                    .musubi_package_invitations
                    .get(&self.invite_id)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi package invitation was not found"))?;
                if invitation.package != self.package
                    || invitation.state != MusubiInvitationStateV1::Pending
                {
                    return Err(invariant(
                        "Musubi package invitation is not pending for this package",
                    ));
                }
                let height = execution_height(state_transaction);
                if height > invitation.expires_at_height {
                    return Err(invariant(
                        "Musubi package invitation has expired; the next successful package governance mutation commits bounded expiry cleanup",
                    ));
                }
                let advance = plan_package_governance_advance(
                    &mut package,
                    height,
                    Some((self.invite_id, MusubiInvitationStateV1::Revoked)),
                    state_transaction.world(),
                )?;
                let next = advance.revision;
                let invitation = advance
                    .terminal_invitation
                    .as_ref()
                    .cloned()
                    .ok_or_else(|| invariant("Musubi revoked invitation was not terminalized"))?;
                let event = MusubiEvent::MaintainerInvitationRevoked(
                    MusubiMaintainerInvitationLifecycleEventV1 {
                        package: self.package.clone(),
                        invite_id: self.invite_id,
                        invited_account: invitation.invited_account.clone(),
                        governance_revision: next,
                        finalized_height: height,
                    },
                );
                state_transaction
                    .world
                    .musubi_packages
                    .insert(self.package, package);
                advance.apply_invitation_updates(state_transaction);
                for event in advance.invitation_events {
                    emit_musubi_event(event, state_transaction);
                }
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for SetMusubiPackageMaintainerRoleV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::SetRole,
            |state_transaction, rejection_reason| {
                validate_role(self.role)?;
                ensure_package_owner(
                    &self.package,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                let mut package = state_transaction
                    .world
                    .musubi_packages
                    .get(&self.package)
                    .cloned()
                    .ok_or_else(|| package_not_found(&self.package))?;
                classify_governance_rejection(
                    ensure_revision(
                        "package governance",
                        package.revisions.governance,
                        self.expected_governance_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                let key = MusubiPackageMemberKeyV1::new(self.package.clone(), self.account.clone());
                let mut member = state_transaction
                    .world
                    .musubi_package_members
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi package member was not found"))?;
                if member.role == self.role {
                    return Err(invariant("Musubi package member role is unchanged"));
                }
                if member.role == MusubiPackageRoleV1::Owner
                    && self.role != MusubiPackageRoleV1::Owner
                    && package.owners.len() == 1
                {
                    return reject_governance_mutation(
                        rejection_reason,
                        MusubiGovernanceRejectionReasonV1::LastOwner,
                        invariant("Musubi package must retain its last owner"),
                    );
                }
                if member.role != MusubiPackageRoleV1::Owner
                    && self.role == MusubiPackageRoleV1::Owner
                    && package.owners.len() >= MUSUBI_MAX_PACKAGE_OWNERS_V1
                {
                    return Err(invariant("Musubi package owner bound is exhausted"));
                }
                if member.role == MusubiPackageRoleV1::Owner
                    && matches!(self.role, MusubiPackageRoleV1::Maintainer(_))
                    && package
                        .member_accounts
                        .len()
                        .saturating_sub(package.owners.len())
                        >= MUSUBI_MAX_PACKAGE_MAINTAINERS_V1
                {
                    return Err(invariant("Musubi package maintainer bound is exhausted"));
                }
                let height = execution_height(state_transaction);
                package.owners.retain(|owner| owner != &self.account);
                if self.role == MusubiPackageRoleV1::Owner {
                    package.owners.push(self.account.clone());
                    package.owners.sort();
                    package.owners.dedup();
                }
                package
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                let advance = plan_package_governance_advance(
                    &mut package,
                    height,
                    None,
                    state_transaction.world(),
                )?;
                let next = advance.revision;
                member.role = self.role;
                member.governance_revision = next;
                let event = MusubiEvent::MaintainerRoleChanged(member.clone());
                state_transaction
                    .world
                    .musubi_packages
                    .insert(self.package, package);
                state_transaction
                    .world
                    .musubi_package_members
                    .insert(key, member.clone());
                upsert_maintainer_directory(
                    MusubiMaintainerDirectoryEntryV1::Accepted(member),
                    state_transaction,
                );
                advance.apply_invitation_updates(state_transaction);
                for event in advance.invitation_events {
                    emit_musubi_event(event, state_transaction);
                }
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for RemoveMusubiPackageMaintainerV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Remove,
            |state_transaction, rejection_reason| {
                ensure_package_owner(
                    &self.package,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                let mut package = state_transaction
                    .world
                    .musubi_packages
                    .get(&self.package)
                    .cloned()
                    .ok_or_else(|| package_not_found(&self.package))?;
                classify_governance_rejection(
                    ensure_revision(
                        "package governance",
                        package.revisions.governance,
                        self.expected_governance_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                let key = MusubiPackageMemberKeyV1::new(self.package.clone(), self.account.clone());
                let member = state_transaction
                    .world
                    .musubi_package_members
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi package member was not found"))?;
                let directory_key = MusubiMaintainerDirectoryKeyV1::accepted(
                    member.package.clone(),
                    member.account.clone(),
                );
                if member.role == MusubiPackageRoleV1::Owner && package.owners.len() == 1 {
                    return reject_governance_mutation(
                        rejection_reason,
                        MusubiGovernanceRejectionReasonV1::LastOwner,
                        invariant("Musubi package must retain its last owner"),
                    );
                }
                package.owners.retain(|owner| owner != &self.account);
                package
                    .member_accounts
                    .retain(|account| account != &self.account);
                let height = execution_height(state_transaction);
                package
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                let advance = plan_package_governance_advance(
                    &mut package,
                    height,
                    None,
                    state_transaction.world(),
                )?;
                let next = advance.revision;
                let event = MusubiEvent::MaintainerRemoved(MusubiPackageMemberRemovedEventV1 {
                    package: self.package.clone(),
                    account: self.account.clone(),
                    previous_role: member.role,
                    governance_revision: next,
                    finalized_height: height,
                });
                state_transaction
                    .world
                    .musubi_packages
                    .insert(self.package, package);
                state_transaction.world.musubi_package_members.remove(key);
                state_transaction
                    .world
                    .musubi_maintainer_directory
                    .remove(directory_key);
                advance.apply_invitation_updates(state_transaction);
                for event in advance.invitation_events {
                    emit_musubi_event(event, state_transaction);
                }
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for RegisterMusubiAliasV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Alias,
            |state_transaction, rejection_reason| {
                self.alias
                    .validate()
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let existing = state_transaction
                    .world
                    .musubi_aliases
                    .get(&self.alias)
                    .cloned();
                if existing
                    .as_ref()
                    .is_some_and(|record| record.target != self.target)
                {
                    return Err(invariant(format!(
                        "Musubi alias '{}' is permanent and already registered",
                        self.alias
                    )));
                }
                if existing.is_some() {
                    ensure_package_owner(
                        &self.target,
                        authority,
                        state_transaction.world(),
                        rejection_reason,
                    )?;
                    return Ok(());
                }
                let policy = state_transaction.world.musubi_registry_policy.get().clone();
                ensure_admitted(
                    &policy,
                    Some(self.target.home_dataspace),
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                if policy.alias_pricing.revision != self.expected_pricing_revision {
                    return reject_governance_mutation(
                        rejection_reason,
                        MusubiGovernanceRejectionReasonV1::Payment,
                        stale_revision(
                            "alias pricing",
                            policy.alias_pricing.revision,
                            self.expected_pricing_revision,
                        ),
                    );
                }
                ensure_package_owner(
                    &self.target,
                    authority,
                    state_transaction.world(),
                    rejection_reason,
                )?;
                if !package_has_active_release(&self.target, state_transaction.world()) {
                    return Err(invariant(
                        "Musubi alias target must have at least one active release",
                    ));
                }
                let price = policy.alias_pricing.price_for(&self.alias);
                let source = AssetId::new(
                    state_transaction.gov.sorafs_pin_fee_asset_id.clone(),
                    authority.clone(),
                );
                let treasury = state_transaction
                    .gov
                    .sorafs_pin_fee_treasury_account
                    .clone();
                classify_governance_rejection(
                    crate::smartcontracts::isi::asset::isi::execute_user_numeric_asset_transfer(
                        state_transaction,
                        authority,
                        source,
                        treasury,
                        Quantity::from(price),
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::Payment,
                )?;
                let height = execution_height(state_transaction);
                let record = MusubiAliasRecordV1 {
                    alias: self.alias.clone(),
                    target: self.target.clone(),
                    registered_by: authority.clone(),
                    pricing_revision: policy.alias_pricing.revision,
                    paid_xor: price,
                    registered_at_height: height,
                    history_revision: 1,
                };
                record
                    .validate(&policy.alias_pricing)
                    .map_err(|error| invariant(error.reason()))?;
                state_transaction
                    .world
                    .musubi_aliases
                    .insert(self.alias.clone(), record);
                let history = MusubiAliasHistoryEntryV1 {
                    alias: self.alias,
                    revision: 1,
                    action: MusubiAliasHistoryActionV1::Registered,
                    previous_target: None,
                    target: self.target,
                    governance_action: None,
                    finalized_height: height,
                };
                let event = MusubiEvent::AliasRegistered(history.clone());
                state_transaction
                    .world
                    .musubi_alias_history
                    .insert(history.key(), history);
                let _ = bump_resolver_index_revision(state_transaction)?;
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for RecoverMusubiPackageV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Recovery,
            |state_transaction, rejection_reason| {
                let action =
                    MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
                        package: self.package.clone(),
                        owners: self.owners.clone(),
                        expected_revision: self.expected_governance_revision,
                    });
                verify_parliament_decision(
                    &self.decision,
                    &action,
                    state_transaction,
                    rejection_reason,
                )?;
                let mut package = state_transaction
                    .world
                    .musubi_packages
                    .get(&self.package)
                    .cloned()
                    .ok_or_else(|| package_not_found(&self.package))?;
                classify_governance_rejection(
                    ensure_revision(
                        "package governance",
                        package.revisions.governance,
                        self.expected_governance_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                let height = execution_height(state_transaction);
                let existing_members = package
                    .member_accounts
                    .iter()
                    .map(|account| {
                        let key =
                            MusubiPackageMemberKeyV1::new(self.package.clone(), account.clone());
                        let member = state_transaction
                            .world
                            .musubi_package_members
                            .get(&key)
                            .cloned()
                            .ok_or_else(|| {
                                invariant("Musubi package member directory is inconsistent")
                            })?;
                        Ok((key, member))
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                for (_, member) in &existing_members {
                    if member.role == MusubiPackageRoleV1::Owner
                        && !self.owners.contains(&member.account)
                    {
                        package
                            .member_accounts
                            .retain(|account| account != &member.account);
                    }
                }
                for owner in &self.owners {
                    if package.member_accounts.binary_search(owner).is_err() {
                        package.member_accounts.push(owner.clone());
                        package.member_accounts.sort();
                        package.member_accounts.dedup();
                    }
                }
                package.owners = self.owners;
                package
                    .validate()
                    .map_err(|error| invariant(error.reason()))?;
                let advance = plan_package_governance_advance(
                    &mut package,
                    height,
                    None,
                    state_transaction.world(),
                )?;
                let next = advance.revision;
                let owner_count = u8::try_from(package.owners.len())
                    .map_err(|_| invariant("Musubi package owner count overflows u8"))?;
                let event = MusubiEvent::PackageRecovered(MusubiPackageRecoveredEventV1 {
                    package: self.package.clone(),
                    action_digest: self.decision.action_digest,
                    owner_count,
                    governance_revision: next,
                    finalized_height: height,
                });
                consume_parliament_decision(self.decision, state_transaction)?;
                for (key, member) in existing_members {
                    if member.role == MusubiPackageRoleV1::Owner
                        && !package.owners.contains(&member.account)
                    {
                        state_transaction.world.musubi_package_members.remove(key);
                        state_transaction.world.musubi_maintainer_directory.remove(
                            MusubiMaintainerDirectoryKeyV1::accepted(
                                member.package,
                                member.account,
                            ),
                        );
                    }
                }
                for owner in &package.owners {
                    let member = MusubiPackageMemberV1 {
                        package: self.package.clone(),
                        account: owner.clone(),
                        role: MusubiPackageRoleV1::Owner,
                        accepted_at_height: height,
                        governance_revision: next,
                    };
                    state_transaction.world.musubi_package_members.insert(
                        MusubiPackageMemberKeyV1::new(self.package.clone(), owner.clone()),
                        member.clone(),
                    );
                    upsert_maintainer_directory(
                        MusubiMaintainerDirectoryEntryV1::Accepted(member),
                        state_transaction,
                    );
                }
                state_transaction
                    .world
                    .musubi_packages
                    .insert(self.package, package);
                advance.apply_invitation_updates(state_transaction);
                for event in advance.invitation_events {
                    emit_musubi_event(event, state_transaction);
                }
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for RetargetMusubiAliasV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Alias,
            |state_transaction, rejection_reason| {
                let action = MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
                    alias: self.alias.clone(),
                    target: self.target.clone(),
                    expected_revision: self.expected_history_revision,
                });
                verify_parliament_decision(
                    &self.decision,
                    &action,
                    state_transaction,
                    rejection_reason,
                )?;
                if state_transaction
                    .world
                    .musubi_packages
                    .get(&self.target)
                    .is_none()
                {
                    return Err(package_not_found(&self.target));
                }
                let mut alias = state_transaction
                    .world
                    .musubi_aliases
                    .get(&self.alias)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi alias was not found"))?;
                classify_governance_rejection(
                    ensure_revision(
                        "alias history",
                        alias.history_revision,
                        self.expected_history_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                if alias.target == self.target {
                    return Err(invariant("Musubi alias retarget is unchanged"));
                }
                let next = next_revision(alias.history_revision, "alias history")?;
                let previous_target = alias.target.clone();
                alias.target = self.target.clone();
                alias.history_revision = next;
                state_transaction
                    .world
                    .musubi_aliases
                    .insert(self.alias.clone(), alias);
                let history = MusubiAliasHistoryEntryV1 {
                    alias: self.alias,
                    revision: next,
                    action: MusubiAliasHistoryActionV1::ParliamentRetarget,
                    previous_target: Some(previous_target),
                    target: self.target,
                    governance_action: Some(self.decision.action_digest),
                    finalized_height: execution_height(state_transaction),
                };
                let event = MusubiEvent::AliasRetargeted(history.clone());
                state_transaction
                    .world
                    .musubi_alias_history
                    .insert(history.key(), history);
                consume_parliament_decision(self.decision, state_transaction)?;
                let _ = bump_resolver_index_revision(state_transaction)?;
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for SetMusubiArtifactTakedownV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Takedown,
            |state_transaction, rejection_reason| {
                let action =
                    MusubiParliamentActionV1::TakedownArtifact(MusubiTakedownArtifactActionV1 {
                        release: self.release.clone(),
                        reason: self.reason.clone(),
                        expected_artifact_governance_revision: self
                            .expected_artifact_governance_revision,
                    });
                verify_parliament_decision(
                    &self.decision,
                    &action,
                    state_transaction,
                    rejection_reason,
                )?;
                let mut release = state_transaction
                    .world
                    .musubi_releases
                    .get(&self.release)
                    .cloned()
                    .ok_or_else(|| release_not_found(&self.release))?;
                classify_governance_rejection(
                    ensure_revision(
                        "artifact governance",
                        release.revisions.artifact_governance,
                        self.expected_artifact_governance_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                if !matches!(
                    release.artifact_governance,
                    MusubiArtifactGovernanceStateV1::Available
                ) {
                    return Err(invariant("Musubi artifact is already taken down"));
                }
                let next =
                    next_revision(release.revisions.artifact_governance, "artifact governance")?;
                let governance =
                    MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
                        action_digest: self.decision.action_digest,
                        reason: self.reason,
                        applied_at_height: execution_height(state_transaction),
                    });
                release.artifact_governance = governance.clone();
                release.revisions.artifact_governance = next;
                let event = MusubiEvent::ArtifactTakenDown(MusubiArtifactTakedownEventV1 {
                    release: self.release.clone(),
                    archive_id: release.manifest.archive_id,
                    action_digest: self.decision.action_digest,
                    governance_revision: next,
                    finalized_height: execution_height(state_transaction),
                });
                let index_revision = bump_resolver_index_revision(state_transaction)?;
                let mut row = state_transaction
                    .world
                    .musubi_resolver_index
                    .get(&self.release)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi release is absent from the resolver index"))?;
                row.selection.governance = governance;
                row.index_revision = index_revision;
                state_transaction
                    .world
                    .musubi_releases
                    .insert(self.release.clone(), release);
                state_transaction
                    .world
                    .musubi_resolver_index
                    .insert(self.release.clone(), row);
                consume_parliament_decision(self.decision, state_transaction)?;
                refresh_directory_for_package(
                    &self.release.package,
                    index_revision,
                    state_transaction,
                )?;
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for SetMusubiRegistryPolicyV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        execute_governance_mutation(
            state_transaction,
            MusubiGovernanceActionV1::Policy,
            |state_transaction, rejection_reason| {
                let action =
                    MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
                        policy: self.policy.clone(),
                        expected_revision: self.expected_policy_revision,
                    });
                verify_parliament_decision(
                    &self.decision,
                    &action,
                    state_transaction,
                    rejection_reason,
                )?;
                let current_policy = state_transaction.world.musubi_registry_policy.get().clone();
                classify_governance_rejection(
                    ensure_revision(
                        "registry policy",
                        current_policy.revision,
                        self.expected_policy_revision,
                    ),
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::StaleRevision,
                )?;
                self.policy
                    .validate_successor(&current_policy)
                    .map_err(|error| invalid_parameter(error.reason()))?;
                let allowlisted_dataspaces =
                    u16::try_from(self.policy.allowlisted_dataspaces.len())
                        .map_err(|_| invariant("Musubi registry allowlist count overflows u16"))?;
                let event = MusubiEvent::RegistryPolicyChanged(MusubiRegistryPolicyEventV1 {
                    revision: self.policy.revision,
                    mode: self.policy.mode,
                    alias_pricing_revision: self.policy.alias_pricing.revision,
                    allowlisted_dataspaces,
                    action_digest: self.decision.action_digest,
                    finalized_height: execution_height(state_transaction),
                });
                *state_transaction.world.musubi_registry_policy.get_mut() = self.policy;
                consume_parliament_decision(self.decision, state_transaction)?;
                let _ = bump_resolver_index_revision(state_transaction)?;
                emit_musubi_event(event, state_transaction);
                Ok(())
            },
        )
    }
}

impl Execute for AssertMusubiReleaseDigestV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let release = state_transaction
            .world
            .musubi_releases
            .get(&self.release)
            .ok_or_else(|| release_not_found(&self.release))?;
        if release.release_digest == self.expected_digest {
            Ok(())
        } else {
            Err(invariant(format!(
                "Musubi release '{}' digest assertion failed",
                self.release
            )))
        }
    }
}

impl ValidSingularQuery for FindMusubiExactPackageV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiPackageRecordV1, QueryExecutionFail> {
        self.request.package.validate().map_err(query_invalid)?;
        state_ro
            .world()
            .musubi_packages()
            .get(&self.request.package)
            .cloned()
            .ok_or(QueryExecutionFail::NotFound)
    }
}

impl ValidSingularQuery for FindMusubiExactReleaseV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiExactReleaseSnapshotV1, QueryExecutionFail> {
        self.request.release.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let chain_id = state_ro.chain_id().clone();
        let genesis_hash = state_ro
            .block_hashes()
            .first()
            .map(|hash| *hash.as_ref())
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "Musubi exact release queries require a finalized genesis block".to_owned(),
                )
            })?;
        let world = state_ro.world();
        let home_release = world.musubi_releases().get(&self.request.release).cloned();
        let universal_release = world
            .musubi_resolver_index()
            .get(&self.request.release)
            .cloned();
        let (home_release, universal_release) = match (home_release, universal_release) {
            (None, None) => return Err(QueryExecutionFail::NotFound),
            (Some(home_release), Some(universal_release)) => (home_release, universal_release),
            (Some(_), None) | (None, Some(_)) => {
                return Err(QueryExecutionFail::Conversion(
                    "Musubi exact release home and universal projections are inconsistent"
                        .to_owned(),
                ));
            }
        };
        let response = MusubiExactReleaseSnapshotV1 {
            chain_id,
            genesis_hash,
            snapshot,
            home_release,
            universal_release,
        };
        response
            .validate_for(&self.request)
            .map_err(query_invalid)?;
        Ok(response)
    }
}

impl ValidSingularQuery for FindMusubiProviderBundleAttestationV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiProviderBundleAttestationRecordV1, QueryExecutionFail> {
        self.key.validate().map_err(query_invalid)?;
        let record = state_ro
            .world()
            .musubi_provider_bundle_attestations()
            .get(&self.key)
            .cloned()
            .ok_or(QueryExecutionFail::NotFound)?;
        record.validate().map_err(query_invalid)?;
        record
            .attestation
            .verify(&record.attestation.payload.binding)
            .map_err(query_invalid)?;
        if record.key != self.key {
            return Err(QueryExecutionFail::Conversion(
                "Musubi provider attestation record has the wrong embedded identity".to_owned(),
            ));
        }
        Ok(record)
    }
}

/// Internal typed Musubi query failure retained through the Torii telemetry boundary.
///
/// The public query error remains [`QueryExecutionFail`]. This wrapper carries
/// a Musubi-only cursor reason in-process without changing the global query
/// wire enum or its variant indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiQueryExecutionErrorV1 {
    query_error: QueryExecutionFail,
    cursor_failure: Option<MusubiCursorFailureV1>,
}

impl MusubiQueryExecutionErrorV1 {
    fn cursor(reason: MusubiCursorFailureV1) -> Self {
        Self {
            query_error: QueryExecutionFail::Expired,
            cursor_failure: Some(reason),
        }
    }

    /// Return the exact typed cursor failure, when this is a cursor error.
    #[must_use]
    pub const fn cursor_failure(&self) -> Option<MusubiCursorFailureV1> {
        self.cursor_failure
    }

    /// Drop the process-local telemetry detail and recover the stable public query error.
    #[must_use]
    pub fn into_query_error(self) -> QueryExecutionFail {
        self.query_error
    }
}

impl From<QueryExecutionFail> for MusubiQueryExecutionErrorV1 {
    fn from(query_error: QueryExecutionFail) -> Self {
        Self {
            query_error,
            cursor_failure: None,
        }
    }
}

/// Execute a paged Musubi query while retaining its exact cursor-failure reason.
///
/// Ordinary Core callers continue to use [`ValidSingularQuery`]. Torii uses
/// this trait only to observe the bounded reason before returning the same
/// public [`QueryExecutionFail`] value.
pub trait ValidMusubiSingularQuery: iroha_data_model::query::SingularQuery {
    /// Execute against one read-only state view with typed cursor diagnostics.
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<Self::Output, MusubiQueryExecutionErrorV1>;
}

macro_rules! impl_valid_singular_query_via_musubi {
    ($query:ty, $output:ty) => {
        impl ValidSingularQuery for $query {
            fn execute(
                &self,
                state_ro: &impl StateReadOnly,
            ) -> Result<$output, QueryExecutionFail> {
                self.execute_musubi(state_ro)
                    .map_err(MusubiQueryExecutionErrorV1::into_query_error)
            }
        }
    };
}

impl ValidMusubiSingularQuery for FindMusubiResolverIndexV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiResolverIndexPageV1, MusubiQueryExecutionErrorV1> {
        self.request.package.validate().map_err(query_invalid)?;
        if let Some(requirement) = &self.request.requirement {
            requirement.validate().map_err(query_invalid)?;
        }
        let snapshot = query_snapshot(state_ro)?;
        let chain_id = state_ro.chain_id().clone();
        let genesis_hash = state_ro
            .block_hashes()
            .first()
            .map(|hash| *hash.as_ref())
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "Musubi resolver queries require a finalized genesis block".to_owned(),
                )
            })?;
        let query_hash = resolver_query_hash(&self.request);
        let start = package_release_page_start(&self.request.package, &self.request.page)?;
        let rows = state_ro
            .world()
            .musubi_resolver_index()
            .range(start..)
            .take_while(|(release, _)| release.package == self.request.package)
            .filter(|(release, _)| {
                self.request
                    .requirement
                    .as_ref()
                    .is_none_or(|requirement| requirement.matches(&release.version))
            })
            .map(|(release, row)| (release.version.to_string(), row.clone()));
        let (items, next_cursor) = paginate(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiResolverIndexPageV1 {
            query: self.request.clone(),
            chain_id,
            genesis_hash,
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiResolverIndexV1, MusubiResolverIndexPageV1);

impl ValidMusubiSingularQuery for FindMusubiVersionsV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiVersionPageV1, MusubiQueryExecutionErrorV1> {
        self.request.package.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let query_hash = package_page_query_hash(b"versions", &self.request);
        let start = package_release_page_start(&self.request.package, &self.request.page)?;
        let rows = state_ro
            .world()
            .musubi_resolver_index()
            .range(start..)
            .take_while(|(release, _)| release.package == self.request.package)
            .map(|(release, _)| (release.version.to_string(), release.version.clone()));
        let (items, next_cursor) = paginate(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiVersionPageV1 {
            query: self.request.clone(),
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiVersionsV1, MusubiVersionPageV1);

impl ValidMusubiSingularQuery for FindMusubiMaintainersV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiMaintainerPageV1, MusubiQueryExecutionErrorV1> {
        self.request.package.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let query_hash = package_page_query_hash(b"maintainers", &self.request);
        state_ro
            .world()
            .musubi_packages()
            .get(&self.request.package)
            .ok_or(QueryExecutionFail::NotFound)?;
        let start = MusubiMaintainerDirectoryKeyV1::package_start(self.request.package.clone());
        let rows = state_ro
            .world()
            .musubi_maintainer_directory()
            .range(start..)
            .take_while(|(key, _)| key.package == self.request.package)
            .filter(|(_, entry)| {
                maintainer_directory_entry_visible_at_height(entry, snapshot.finalized_height)
            })
            .map(|(_, entry)| (entry.cursor_key(), entry.clone()));
        let (items, next_cursor) = paginate(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiMaintainerPageV1 {
            query: self.request.clone(),
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiMaintainersV1, MusubiMaintainerPageV1);

impl ValidMusubiSingularQuery for FindMusubiArchiveLocationsV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiArchiveLocationPageV1, MusubiQueryExecutionErrorV1> {
        if self.request.archive_id.is_zero() {
            return Err(QueryExecutionFail::Conversion(
                "Musubi archive id must not be zero".to_owned(),
            )
            .into());
        }
        let snapshot = query_snapshot(state_ro)?;
        let chain_id = state_ro.chain_id().clone();
        let genesis_hash = state_ro
            .block_hashes()
            .first()
            .map(|hash| *hash.as_ref())
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "Musubi archive-location queries require a finalized genesis block".to_owned(),
                )
            })?;
        let query_hash = archive_location_query_hash(&self.request);
        let archive = state_ro
            .world()
            .musubi_archives()
            .get(&self.request.archive_id)
            .cloned()
            .ok_or(QueryExecutionFail::NotFound)?;
        let rows = archive
            .location_ids
            .iter()
            .map(|location_id| {
                let key = MusubiArchiveLocationKeyV1::new(self.request.archive_id, *location_id);
                let location = state_ro
                    .world()
                    .musubi_archive_locations()
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| {
                        QueryExecutionFail::Conversion(
                            "Musubi archive location directory is inconsistent".to_owned(),
                        )
                    })?;
                Ok((
                    format!(
                        "{}:{}",
                        digest_label(key.archive_id.as_bytes()),
                        digest_label(key.location_id.as_bytes())
                    ),
                    location,
                ))
            })
            .collect::<Result<Vec<_>, QueryExecutionFail>>()?;
        let (items, next_cursor) = paginate(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiArchiveLocationPageV1 {
            chain_id,
            genesis_hash,
            archive,
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiArchiveLocationsV1, MusubiArchiveLocationPageV1);

impl ValidSingularQuery for FindMusubiArchiveRetentionV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiArchiveRetentionPageV1, QueryExecutionFail> {
        self.request.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        if self
            .request
            .expected_snapshot
            .is_some_and(|expected| expected != snapshot)
        {
            return Err(QueryExecutionFail::Expired);
        }
        let chain_id = state_ro.chain_id().clone();
        let genesis_hash = state_ro
            .block_hashes()
            .first()
            .map(|hash| *hash.as_ref())
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "Musubi archive-retention queries require a finalized genesis block".to_owned(),
                )
            })?;
        let world = state_ro.world();
        let mut items = Vec::with_capacity(self.request.archive_ids.len());
        for archive_id in &self.request.archive_ids {
            items.push(archive_retention_decision(*archive_id, world)?);
        }
        let finalized_block = state_ro.latest_block().ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "Musubi finalized snapshot block body is unavailable".to_owned(),
            )
        })?;
        let finalized_time_ms = validated_finalized_block_time(
            &snapshot,
            finalized_block.header().height().get(),
            *finalized_block.hash().as_ref(),
            finalized_block.header().creation_time_ms,
        )?;
        let page = MusubiArchiveRetentionPageV1 {
            chain_id,
            genesis_hash,
            items,
            snapshot,
            finalized_time_ms,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}

fn validated_finalized_block_time(
    snapshot: &MusubiRegistrySnapshotV1,
    block_height: u64,
    block_hash: [u8; 32],
    creation_time_ms: u64,
) -> Result<u64, QueryExecutionFail> {
    if block_height != snapshot.finalized_height || block_hash != snapshot.finalized_block_hash {
        return Err(QueryExecutionFail::Conversion(
            "Musubi finalized snapshot does not match its block body".to_owned(),
        ));
    }
    Ok(creation_time_ms)
}

fn archive_retention_decision(
    archive_id: ArchiveId,
    world: &impl WorldReadOnly,
) -> Result<MusubiArchiveRetentionDecisionV1, QueryExecutionFail> {
    let archive = world.musubi_archives().get(&archive_id).cloned();
    let references = world.musubi_archive_reverse_references().get(&archive_id);
    let storage = world.musubi_archive_availability().get(&archive_id);
    let Some(archive) = archive else {
        if references.is_some() || storage.is_some() {
            return Err(QueryExecutionFail::Conversion(
                "Musubi archive retention state has an orphan universal projection".to_owned(),
            ));
        }
        return Ok(MusubiArchiveRetentionDecisionV1 {
            archive_id,
            disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
            active_releases: 0,
            yanked_releases: 0,
            taken_down_releases: 0,
            storage: None,
        });
    };
    archive.validate().map_err(query_invalid)?;
    if archive.archive_id != archive_id {
        return Err(QueryExecutionFail::Conversion(
            "Musubi archive retention storage key disagrees with its archive record".to_owned(),
        ));
    }
    let references = references.ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "Musubi archive retention state is missing reverse references".to_owned(),
        )
    })?;
    references.validate().map_err(query_invalid)?;
    if references.archive_id != archive_id {
        return Err(QueryExecutionFail::Conversion(
            "Musubi archive retention reverse-reference identity is inconsistent".to_owned(),
        ));
    }
    let storage = storage.cloned().ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "Musubi archive retention state is missing storage availability".to_owned(),
        )
    })?;
    storage.validate().map_err(query_invalid)?;
    if storage.archive_id != archive_id {
        return Err(QueryExecutionFail::Conversion(
            "Musubi archive retention storage identity is inconsistent".to_owned(),
        ));
    }

    let mut active_releases = 0_u16;
    let mut yanked_releases = 0_u16;
    let mut taken_down_releases = 0_u16;
    for release_id in &references.releases {
        let release = world.musubi_releases().get(release_id).ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "Musubi archive retention reference names a missing release".to_owned(),
            )
        })?;
        release.validate().map_err(query_invalid)?;
        if release.manifest.release != *release_id || release.manifest.archive_id != archive_id {
            return Err(QueryExecutionFail::Conversion(
                "Musubi archive retention reference disagrees with its release".to_owned(),
            ));
        }
        match &release.artifact_governance {
            MusubiArtifactGovernanceStateV1::Available if release.yank.yanked => {
                yanked_releases = yanked_releases.checked_add(1).ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "Musubi archive retention yanked-release count overflow".to_owned(),
                    )
                })?;
            }
            MusubiArtifactGovernanceStateV1::Available => {
                active_releases = active_releases.checked_add(1).ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "Musubi archive retention active-release count overflow".to_owned(),
                    )
                })?;
            }
            MusubiArtifactGovernanceStateV1::TakenDown(_) => {
                taken_down_releases = taken_down_releases.checked_add(1).ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "Musubi archive retention takedown count overflow".to_owned(),
                    )
                })?;
            }
        }
    }
    let disposition = if active_releases > 0 || yanked_releases > 0 {
        MusubiArchiveRetentionDispositionV1::RetainReferenced
    } else if taken_down_releases > 0 {
        MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown
    } else {
        MusubiArchiveRetentionDispositionV1::PruneUnreferenced
    };
    let decision = MusubiArchiveRetentionDecisionV1 {
        archive_id,
        disposition,
        active_releases,
        yanked_releases,
        taken_down_releases,
        storage: Some(storage),
    };
    decision.validate().map_err(query_invalid)?;
    Ok(decision)
}

impl ValidSingularQuery for FindMusubiAliasV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiAliasRecordV1, QueryExecutionFail> {
        self.request.alias.validate().map_err(query_invalid)?;
        state_ro
            .world()
            .musubi_aliases()
            .get(&self.request.alias)
            .cloned()
            .ok_or(QueryExecutionFail::NotFound)
    }
}

impl ValidMusubiSingularQuery for FindMusubiAliasHistoryV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiAliasHistoryPageV1, MusubiQueryExecutionErrorV1> {
        self.request.alias.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let query_hash = alias_history_query_hash(&self.request);
        let start = alias_history_page_start(&self.request)?;
        let rows = state_ro
            .world()
            .musubi_alias_history()
            .range(start..)
            .take_while(|(key, _)| key.alias == self.request.alias)
            .map(|(key, history)| {
                (
                    format!("{}:{:020}", key.alias, key.revision),
                    history.clone(),
                )
            });
        let (items, next_cursor) = paginate(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiAliasHistoryPageV1 {
            query: self.request.clone(),
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiAliasHistoryV1, MusubiAliasHistoryPageV1);

impl ValidMusubiSingularQuery for FindMusubiOrderedPrefixV1 {
    fn execute_musubi(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiOrderedPackagePageV1, MusubiQueryExecutionErrorV1> {
        self.request.prefix.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let chain_id = state_ro.chain_id().clone();
        let genesis_hash = state_ro
            .block_hashes()
            .first()
            .map(|hash| *hash.as_ref())
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "Musubi directory queries require a finalized genesis block".to_owned(),
                )
            })?;
        let query_hash = ordered_prefix_query_hash(&self.request);
        let prefix = self.request.prefix.as_str();
        let (start, namespace, name_prefix) = directory_query_start(&self.request)?;
        let namespace_binding = state_ro
            .world()
            .musubi_namespace_bindings()
            .get(&namespace)
            .cloned()
            .ok_or(QueryExecutionFail::NotFound)?;
        let rows = state_ro
            .world()
            .musubi_public_directory()
            .range(start..)
            .take_while(|(selector, _)| {
                selector.namespace == namespace
                    && selector.name.as_str().starts_with(name_prefix.as_str())
            })
            .filter(|(selector, _)| selector.to_string().starts_with(prefix))
            .map(|(selector, entry)| (selector.to_string(), entry.clone()));
        let (items, next_cursor) = paginate(rows, &self.request.page, query_hash, snapshot)?;
        let page = MusubiOrderedPackagePageV1 {
            query: self.request.clone(),
            chain_id,
            genesis_hash,
            namespace_binding,
            items,
            next_cursor,
            snapshot,
        };
        page.validate().map_err(query_invalid)?;
        Ok(page)
    }
}
impl_valid_singular_query_via_musubi!(FindMusubiOrderedPrefixV1, MusubiOrderedPackagePageV1);

fn query_snapshot(
    state_ro: &impl StateReadOnly,
) -> Result<MusubiRegistrySnapshotV1, QueryExecutionFail> {
    let finalized_height = u64::try_from(state_ro.block_hashes().len()).map_err(|_| {
        QueryExecutionFail::Conversion("Musubi finalized height overflows u64".to_owned())
    })?;
    let finalized_block_hash = state_ro
        .block_hashes()
        .last()
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| {
            QueryExecutionFail::Conversion(
                "Musubi queries require at least one finalized block".to_owned(),
            )
        })?;
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height,
        finalized_block_hash,
        index_revision: state_ro.world().musubi_resolver_index_revision(),
    };
    snapshot.validate().map_err(query_invalid)?;
    Ok(snapshot)
}

fn resolver_query_hash(request: &MusubiResolverIndexQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(b"resolver-index", &canonical.encode())
}

fn package_page_query_hash(kind: &[u8], request: &MusubiPackagePageQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(kind, &canonical.encode())
}

fn archive_location_query_hash(request: &MusubiArchiveLocationQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(b"archive-locations", &canonical.encode())
}

fn alias_history_query_hash(request: &MusubiAliasQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(b"alias-history", &canonical.encode())
}

fn alias_history_page_start(
    request: &MusubiAliasQueryV1,
) -> Result<MusubiAliasHistoryKeyV1, MusubiQueryExecutionErrorV1> {
    let revision = if let Some(cursor) = &request.page.cursor {
        let (alias, revision) = cursor.last_key.rsplit_once(':').ok_or_else(|| {
            MusubiQueryExecutionErrorV1::cursor(MusubiCursorFailureV1::LastKeyStale)
        })?;
        if alias != request.alias.as_str() || revision.len() != 20 {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::LastKeyStale,
            ));
        }
        revision
            .parse::<u64>()
            .map_err(|_| MusubiQueryExecutionErrorV1::cursor(MusubiCursorFailureV1::LastKeyStale))?
    } else {
        0
    };
    Ok(MusubiAliasHistoryKeyV1::new(
        request.alias.clone(),
        revision,
    ))
}

fn ordered_prefix_query_hash(request: &MusubiOrderedPrefixQueryV1) -> MusubiQueryHashV1 {
    let mut canonical = request.clone();
    canonical.page.cursor = None;
    query_hash(b"ordered-prefix", &canonical.encode())
}

fn directory_query_start(
    request: &MusubiOrderedPrefixQueryV1,
) -> Result<(MusubiPackageSelectorV1, MusubiNamespaceV1, String), MusubiQueryExecutionErrorV1> {
    let raw = request.prefix.as_str();
    let (namespace_raw, name_prefix) = raw.split_once('/').ok_or_else(|| {
        QueryExecutionFail::Conversion(
            "Musubi ordered directory prefix must use namespace/package-prefix".to_owned(),
        )
    })?;
    if name_prefix.contains('/')
        || name_prefix.len() > MUSUBI_MAX_PACKAGE_NAME_BYTES_V1
        || name_prefix.starts_with('-')
        || name_prefix.contains("--")
        || !name_prefix
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(QueryExecutionFail::Conversion(
            "Musubi ordered directory package prefix is not portable canonical text".to_owned(),
        )
        .into());
    }
    let namespace = namespace_raw
        .parse::<MusubiNamespaceV1>()
        .map_err(query_invalid)?;
    let start = if let Some(cursor) = &request.page.cursor {
        let selector = cursor
            .last_key
            .parse::<MusubiPackageSelectorV1>()
            .map_err(|_| {
                MusubiQueryExecutionErrorV1::cursor(MusubiCursorFailureV1::LastKeyStale)
            })?;
        if selector.namespace != namespace || !selector.name.as_str().starts_with(name_prefix) {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::LastKeyStale,
            ));
        }
        selector
    } else {
        let lower_name = if name_prefix.is_empty() {
            "0".to_owned()
        } else if name_prefix.ends_with('-') {
            if name_prefix.len() == MUSUBI_MAX_PACKAGE_NAME_BYTES_V1 {
                return Err(QueryExecutionFail::Conversion(
                    "Musubi ordered directory package prefix cannot match a package".to_owned(),
                )
                .into());
            }
            format!("{name_prefix}0")
        } else {
            name_prefix.to_owned()
        };
        MusubiPackageSelectorV1 {
            namespace: namespace.clone(),
            name: lower_name.parse().map_err(query_invalid)?,
        }
    };
    Ok((start, namespace, name_prefix.to_owned()))
}

fn query_hash(domain: &[u8], encoded: &[u8]) -> MusubiQueryHashV1 {
    let mut payload = Vec::with_capacity(domain.len() + encoded.len() + 16);
    payload.extend_from_slice(
        &u64::try_from(domain.len())
            .expect("static Musubi query domain length fits u64")
            .to_le_bytes(),
    );
    payload.extend_from_slice(domain);
    payload.extend_from_slice(
        &u64::try_from(encoded.len())
            .expect("bounded Musubi query length fits u64")
            .to_le_bytes(),
    );
    payload.extend_from_slice(encoded);
    MusubiQueryHashV1::new(*Hash::new(&payload).as_ref())
}

fn paginate<T>(
    rows: impl IntoIterator<Item = (String, T)>,
    page: &MusubiPageRequestV1,
    query_hash: MusubiQueryHashV1,
    snapshot: MusubiRegistrySnapshotV1,
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1> {
    paginate_for_caller(rows, page, query_hash, snapshot, None)
}

fn paginate_for_caller<T>(
    rows: impl IntoIterator<Item = (String, T)>,
    page: &MusubiPageRequestV1,
    query_hash: MusubiQueryHashV1,
    snapshot: MusubiRegistrySnapshotV1,
    expected_caller: Option<&AccountId>,
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1> {
    page.validate().map_err(query_invalid)?;
    let cursor_last_key = if let Some(cursor) = &page.cursor {
        if cursor.snapshot.finalized_height != snapshot.finalized_height
            || cursor.snapshot.finalized_block_hash != snapshot.finalized_block_hash
        {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::FinalizedAnchorMismatch,
            ));
        }
        if cursor.snapshot.index_revision != snapshot.index_revision {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::IndexRevisionMismatch,
            ));
        }
        if cursor.query_hash != query_hash {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::QueryMismatch,
            ));
        }
        if cursor.caller.as_ref() != expected_caller {
            return Err(MusubiQueryExecutionErrorV1::cursor(
                MusubiCursorFailureV1::CallerMismatch,
            ));
        }
        Some(cursor.last_key.as_str())
    } else {
        None
    };
    let limit = page.effective_limit();
    let mut cursor_seen = cursor_last_key.is_none();
    let mut page_rows = Vec::with_capacity(limit.saturating_add(1));
    for (key, item) in rows {
        if !cursor_seen {
            if Some(key.as_str()) == cursor_last_key {
                cursor_seen = true;
            }
            continue;
        }
        page_rows.push((key, item));
        if page_rows.len() > limit {
            break;
        }
    }
    if !cursor_seen {
        return Err(MusubiQueryExecutionErrorV1::cursor(
            MusubiCursorFailureV1::LastKeyStale,
        ));
    }
    let has_more = page_rows.len() > limit;
    if has_more {
        page_rows.pop();
    }
    let last_key = page_rows.last().map(|(key, _)| key.clone());
    let items = page_rows
        .into_iter()
        .map(|(_, item)| item)
        .collect::<Vec<_>>();
    let next_cursor = if has_more {
        Some(MusubiFinalizedCursorV1 {
            snapshot,
            query_hash,
            last_key: last_key.expect("a page with a successor has at least one item"),
            caller: expected_caller.cloned(),
        })
    } else {
        None
    };
    Ok((items, next_cursor))
}

fn query_invalid(error: iroha_data_model::ParseError) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}

fn upsert_maintainer_directory(
    entry: MusubiMaintainerDirectoryEntryV1,
    state_transaction: &mut StateTransaction<'_, '_>,
) {
    state_transaction
        .world
        .musubi_maintainer_directory
        .insert(entry.key(), entry);
}

#[must_use = "a package-governance advance plan must be applied after fallible checks complete"]
struct PackageGovernanceAdvance {
    revision: u64,
    invitation_updates: Vec<PackageInvitationUpdate>,
    invitation_events: Vec<MusubiEvent>,
    terminal_invitation: Option<MusubiMaintainerInvitationV1>,
}

struct PackageInvitationUpdate {
    directory_key: MusubiMaintainerDirectoryKeyV1,
    invitation: MusubiMaintainerInvitationV1,
    keep_pending: bool,
}

impl PackageGovernanceAdvance {
    fn apply_invitation_updates(&self, state_transaction: &mut StateTransaction<'_, '_>) {
        for update in &self.invitation_updates {
            state_transaction
                .world
                .musubi_package_invitations
                .insert(update.invitation.invite_id, update.invitation.clone());
            if update.keep_pending {
                upsert_maintainer_directory(
                    MusubiMaintainerDirectoryEntryV1::PendingInvitation(update.invitation.clone()),
                    state_transaction,
                );
            } else {
                state_transaction
                    .world
                    .musubi_maintainer_directory
                    .remove(update.directory_key.clone());
            }
        }
    }
}

fn plan_package_governance_advance(
    package: &mut MusubiPackageRecordV1,
    finalized_height: u64,
    target_transition: Option<(MusubiInviteIdV1, MusubiInvitationStateV1)>,
    world: &impl WorldReadOnly,
) -> Result<PackageGovernanceAdvance, Error> {
    if target_transition.as_ref().is_some_and(|(_, state)| {
        !matches!(
            state,
            MusubiInvitationStateV1::Accepted | MusubiInvitationStateV1::Revoked
        )
    }) {
        return Err(invariant(
            "Musubi invitation target transition must be terminal",
        ));
    }
    let previous_revision = package.revisions.governance;
    let revision = next_revision(previous_revision, "package governance")?;
    let directory_bound = MUSUBI_MAX_PACKAGE_MEMBERS_V1
        .checked_add(MUSUBI_MAX_PENDING_INVITATIONS_V1)
        .expect("bounded Musubi maintainer directory size fits usize");
    let entries = world
        .musubi_maintainer_directory()
        .range(MusubiMaintainerDirectoryKeyV1::package_start(package.package.clone())..)
        .take_while(|(key, _)| key.package == package.package)
        .take(directory_bound.saturating_add(1))
        .map(|(key, entry)| (key.clone(), entry.clone()))
        .collect::<Vec<_>>();
    if entries.len() > directory_bound {
        return Err(invariant(
            "Musubi maintainer directory exceeds its package-local bound",
        ));
    }
    let pending = entries
        .into_iter()
        .filter_map(|(key, entry)| match entry {
            MusubiMaintainerDirectoryEntryV1::Accepted(_) => None,
            MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation) => {
                Some((key, invitation))
            }
        })
        .collect::<Vec<_>>();
    if pending.len() > MUSUBI_MAX_PENDING_INVITATIONS_V1 {
        return Err(invariant(
            "Musubi package exceeds the pending-invitation bound",
        ));
    }
    for (directory_key, invitation) in &pending {
        invitation
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        let expected_key = MusubiMaintainerDirectoryKeyV1::pending(
            invitation.package.clone(),
            invitation.invited_account.clone(),
            invitation.invite_id,
        );
        if directory_key != &expected_key
            || invitation.package != package.package
            || invitation.state != MusubiInvitationStateV1::Pending
            || invitation.expected_governance_revision != previous_revision
            || world
                .musubi_package_invitations()
                .get(&invitation.invite_id)
                != Some(invitation)
        {
            return Err(invariant(
                "Musubi pending-invitation directory is inconsistent with package governance",
            ));
        }
    }
    if let Some((target_id, _)) = target_transition.as_ref() {
        let target = pending
            .iter()
            .find(|(_, invitation)| invitation.invite_id == *target_id)
            .map(|(_, invitation)| invitation)
            .ok_or_else(|| {
                invariant("Musubi target invitation is absent from the pending directory")
            })?;
        if target.expires_at_height < finalized_height {
            return Err(invariant(
                "Musubi target invitation expired before its terminal transition",
            ));
        }
    }

    let mut updates = Vec::with_capacity(pending.len());
    let mut invitation_events = Vec::new();
    let mut terminal_invitation = None;
    for (directory_key, mut invitation) in pending {
        invitation.expected_governance_revision = revision;
        let keep_pending = if let Some((target_id, target_state)) = target_transition
            && invitation.invite_id == target_id
        {
            invitation.state = target_state;
            terminal_invitation = Some(invitation.clone());
            false
        } else if invitation.expires_at_height < finalized_height {
            invitation.state = MusubiInvitationStateV1::Expired;
            invitation_events.push(MusubiEvent::MaintainerInvitationExpired(
                MusubiMaintainerInvitationLifecycleEventV1 {
                    package: invitation.package.clone(),
                    invite_id: invitation.invite_id,
                    invited_account: invitation.invited_account.clone(),
                    governance_revision: revision,
                    finalized_height,
                },
            ));
            false
        } else {
            true
        };
        invitation
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        updates.push(PackageInvitationUpdate {
            directory_key,
            invitation,
            keep_pending,
        });
    }
    package.revisions.governance = revision;
    Ok(PackageGovernanceAdvance {
        revision,
        invitation_updates: updates,
        invitation_events,
        terminal_invitation,
    })
}

fn active_pending_invitation_count(
    package: &MusubiPackageIdV1,
    current_height: u64,
    world: &impl WorldReadOnly,
) -> usize {
    world
        .musubi_maintainer_directory()
        .range(MusubiMaintainerDirectoryKeyV1::package_start(package.clone())..)
        .take_while(|(key, _)| &key.package == package)
        .take(MUSUBI_MAX_PACKAGE_MEMBERS_V1 + MUSUBI_MAX_PENDING_INVITATIONS_V1)
        .filter(|(_, entry)| {
            matches!(
                entry,
                MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation)
                    if invitation.state == MusubiInvitationStateV1::Pending
                        && invitation.expires_at_height >= current_height
            )
        })
        .count()
}

#[cfg(test)]
fn pending_invitation_count(package: &MusubiPackageIdV1, world: &impl WorldReadOnly) -> usize {
    world
        .musubi_maintainer_directory()
        .range(MusubiMaintainerDirectoryKeyV1::package_start(package.clone())..)
        .take_while(|(key, _)| &key.package == package)
        .take(MUSUBI_MAX_PACKAGE_MEMBERS_V1 + MUSUBI_MAX_PENDING_INVITATIONS_V1)
        .filter(|(key, _)| key.invitation.is_some())
        .count()
}

fn maintainer_directory_entry_visible_at_height(
    entry: &MusubiMaintainerDirectoryEntryV1,
    finalized_height: u64,
) -> bool {
    match entry {
        MusubiMaintainerDirectoryEntryV1::Accepted(_) => true,
        MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation) => {
            invitation.expires_at_height >= finalized_height
        }
    }
}

#[derive(Clone, Copy)]
enum PackageCapability {
    Publish,
    Yank,
    Metadata,
    ArchiveLocations,
}

fn execute_governance_mutation<'block, 'state>(
    state_transaction: &mut StateTransaction<'block, 'state>,
    action: MusubiGovernanceActionV1,
    execute: impl FnOnce(
        &mut StateTransaction<'block, 'state>,
        &mut MusubiGovernanceRejectionReasonV1,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut rejection_reason = MusubiGovernanceRejectionReasonV1::Other;
    let result = execute(state_transaction, &mut rejection_reason);
    #[cfg(feature = "telemetry")]
    {
        if result.is_err() {
            state_transaction
                .telemetry
                .record_musubi_governance_rejection(action, rejection_reason);
        }
    }
    #[cfg(not(feature = "telemetry"))]
    let _ = action;
    result
}

fn classify_governance_rejection<T>(
    result: Result<T, Error>,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
    reason: MusubiGovernanceRejectionReasonV1,
) -> Result<T, Error> {
    result.map_err(|error| {
        *rejection_reason = reason;
        error
    })
}

fn reject_governance_mutation<T>(
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
    reason: MusubiGovernanceRejectionReasonV1,
    error: Error,
) -> Result<T, Error> {
    *rejection_reason = reason;
    Err(error)
}

fn validate_role(role: MusubiPackageRoleV1) -> Result<(), Error> {
    if matches!(role, MusubiPackageRoleV1::Maintainer(permissions) if permissions.is_empty()) {
        Err(invalid_parameter(
            "Musubi maintainer role must grant at least one permission",
        ))
    } else {
        Ok(())
    }
}

fn ensure_package_owner(
    package: &MusubiPackageIdV1,
    authority: &AccountId,
    world: &impl WorldReadOnly,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    let record = world
        .musubi_packages()
        .get(package)
        .ok_or_else(|| package_not_found(package))?;
    if record.owners.binary_search(authority).is_ok() {
        Ok(())
    } else {
        *rejection_reason = MusubiGovernanceRejectionReasonV1::Unauthorized;
        Err(invariant(format!(
            "authority '{authority}' is not an owner of Musubi package '{package}'"
        )))
    }
}

fn ensure_package_capability(
    package: &MusubiPackageIdV1,
    authority: &AccountId,
    capability: PackageCapability,
    world: &impl WorldReadOnly,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    let record = world
        .musubi_packages()
        .get(package)
        .ok_or_else(|| package_not_found(package))?;
    if record.owners.binary_search(authority).is_ok() {
        return Ok(());
    }
    let key = MusubiPackageMemberKeyV1::new(package.clone(), authority.clone());
    let permitted = world
        .musubi_package_members()
        .get(&key)
        .is_some_and(|member| match member.role {
            MusubiPackageRoleV1::Owner => true,
            MusubiPackageRoleV1::Maintainer(permissions) => match capability {
                PackageCapability::Publish => permissions.publish,
                PackageCapability::Yank => permissions.yank,
                PackageCapability::Metadata => permissions.metadata,
                PackageCapability::ArchiveLocations => permissions.archive_locations,
            },
        });
    if permitted {
        Ok(())
    } else {
        *rejection_reason = MusubiGovernanceRejectionReasonV1::Unauthorized;
        Err(invariant(format!(
            "authority '{authority}' lacks the required Musubi package capability for '{package}'"
        )))
    }
}

fn ensure_archive_manager(
    archive: &MusubiArchiveRecordV1,
    authority: &AccountId,
    world: &impl WorldReadOnly,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    let references = world
        .musubi_archive_reverse_references()
        .get(&archive.archive_id);
    let Some(references) = references.filter(|references| !references.releases.is_empty()) else {
        if &archive.registered_by == authority {
            return Ok(());
        }
        *rejection_reason = MusubiGovernanceRejectionReasonV1::Unauthorized;
        return Err(invariant("authority cannot manage this Musubi archive"));
    };
    for release in &references.releases {
        ensure_package_capability(
            &release.package,
            authority,
            PackageCapability::ArchiveLocations,
            world,
            rejection_reason,
        )?;
    }
    Ok(())
}

fn ensure_namespace_registration_owner(
    binding: &MusubiNamespaceBindingV1,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    let (owner, generation) = namespace_owner_and_generation(binding, state_transaction)?;
    binding
        .validate_authority_generation(generation)
        .map_err(|error| invalid_parameter(error.reason()))?;
    ensure_resolved_namespace_owner(binding, authority, &owner, rejection_reason)
}

fn ensure_namespace_current_owner(
    binding: &MusubiNamespaceBindingV1,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    let (owner, _) = namespace_owner_and_generation(binding, state_transaction)?;
    ensure_resolved_namespace_owner(binding, authority, &owner, rejection_reason)
}

fn ensure_resolved_namespace_owner(
    binding: &MusubiNamespaceBindingV1,
    authority: &AccountId,
    owner: &AccountId,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    if owner == authority {
        Ok(())
    } else {
        *rejection_reason = MusubiGovernanceRejectionReasonV1::Unauthorized;
        Err(invariant(format!(
            "authority '{authority}' does not own Musubi namespace '{}'",
            binding.namespace
        )))
    }
}

fn namespace_owner_and_generation(
    binding: &MusubiNamespaceBindingV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(AccountId, u64), Error> {
    validate_namespace_home_dataspace(
        binding,
        state_transaction.world(),
        &state_transaction.nexus.dataspace_catalog,
        state_transaction.block_unix_timestamp_ms(),
    )?;
    match &binding.scope {
        MusubiPackageScopeV1::Domain(domain) => {
            let domain_id =
                DomainId::try_new(domain.as_ref(), binding.namespace.dataspace_segment())
                    .map_err(|error| invalid_parameter(error.reason()))?;
            let owner = state_transaction
                .world
                .domains()
                .get(&domain_id)
                .map(|registered| registered.owned_by().clone())
                .ok_or_else(|| {
                    invariant(format!(
                        "Musubi namespace domain '{domain_id}' is not registered"
                    ))
                })?;
            let generation = state_transaction
                .world
                .musubi_domain_ownership_generation(&domain_id);
            Ok((owner, generation))
        }
        MusubiPackageScopeV1::DataspaceRoot => {
            crate::sns::active_dataspace_owner_and_generation_by_alias(
                state_transaction.world(),
                binding.namespace.dataspace_segment(),
                state_transaction.block_unix_timestamp_ms(),
            )
            .ok_or_else(|| {
                invariant(format!(
                    "Musubi namespace dataspace '{}' has no active SNS owner",
                    binding.namespace.dataspace_segment()
                ))
            })
        }
    }
}

fn validate_namespace_home_dataspace(
    binding: &MusubiNamespaceBindingV1,
    world: &impl WorldReadOnly,
    catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    current_time_ms: u64,
) -> Result<(), Error> {
    let alias = binding.namespace.dataspace_segment();
    let resolved = crate::sns::resolve_active_dataspace_id_by_alias(
        world,
        catalog,
        alias,
        current_time_ms,
    )
    .map_err(|error| {
        invariant(format!(
            "Musubi namespace dataspace alias '{alias}' cannot be resolved canonically: {error}"
        ))
    })?;
    if resolved != binding.home_dataspace {
        return Err(invariant(format!(
            "Musubi namespace dataspace alias '{alias}' resolves to {resolved}, not declared home dataspace {}",
            binding.home_dataspace
        )));
    }
    Ok(())
}

fn ensure_namespace_claim_authority(
    binding: &MusubiNamespaceBindingV1,
    delegation: Option<&MusubiNamespaceDelegationV1>,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    let (owner, generation) = namespace_owner_and_generation(binding, state_transaction)?;
    validate_namespace_claim_authority_classified(
        binding,
        delegation,
        authority,
        &owner,
        generation,
        execution_height(state_transaction),
        Some(rejection_reason),
    )
}

#[cfg(test)]
fn validate_namespace_claim_authority(
    binding: &MusubiNamespaceBindingV1,
    delegation: Option<&MusubiNamespaceDelegationV1>,
    authority: &AccountId,
    authoritative_owner: &AccountId,
    authoritative_owner_generation: u64,
    current_height: u64,
) -> Result<(), Error> {
    validate_namespace_claim_authority_classified(
        binding,
        delegation,
        authority,
        authoritative_owner,
        authoritative_owner_generation,
        current_height,
        None,
    )
}

fn validate_namespace_claim_authority_classified(
    binding: &MusubiNamespaceBindingV1,
    delegation: Option<&MusubiNamespaceDelegationV1>,
    authority: &AccountId,
    authoritative_owner: &AccountId,
    authoritative_owner_generation: u64,
    current_height: u64,
    mut rejection_reason: Option<&mut MusubiGovernanceRejectionReasonV1>,
) -> Result<(), Error> {
    binding
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if authoritative_owner_generation == 0 {
        return Err(invariant(
            "Musubi namespace authoritative ownership generation must be non-zero",
        ));
    }
    if authoritative_owner == authority {
        return Ok(());
    }
    let delegation = delegation.ok_or_else(|| {
        if let Some(reason) = rejection_reason.as_mut() {
            **reason = MusubiGovernanceRejectionReasonV1::Unauthorized;
        }
        invariant(format!(
            "authority '{authority}' neither owns nor has a delegation for Musubi namespace '{}'",
            binding.namespace
        ))
    })?;
    delegation
        .verify(
            binding,
            authoritative_owner,
            authoritative_owner_generation,
            authority,
            current_height,
        )
        .map_err(|error| {
            if let Some(reason) = rejection_reason.as_mut() {
                **reason = MusubiGovernanceRejectionReasonV1::Unauthorized;
            }
            invariant(error.reason())
        })
}

fn namespace_binding_for_package(
    namespace: &MusubiNamespaceV1,
    package: &MusubiPackageIdV1,
    world: &impl WorldReadOnly,
) -> Result<MusubiNamespaceBindingV1, Error> {
    let binding = world
        .musubi_namespace_bindings()
        .get(namespace)
        .cloned()
        .ok_or_else(|| {
            invariant(format!(
                "Musubi namespace '{namespace}' has no immutable binding"
            ))
        })?;
    if binding.home_dataspace != package.home_dataspace || binding.scope != package.scope {
        return Err(invariant(format!(
            "Musubi namespace '{namespace}' does not identify package '{package}'"
        )));
    }
    Ok(binding)
}

fn ensure_policy_revision(policy: &MusubiRegistryPolicyV1, expected: u64) -> Result<(), Error> {
    ensure_revision("registry policy", policy.revision, expected)
}

fn ensure_admitted(
    policy: &MusubiRegistryPolicyV1,
    dataspace: Option<DataSpaceId>,
    authority: &AccountId,
    world: &impl WorldReadOnly,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    match policy.mode {
        MusubiRegistryAdmissionModeV1::Open => Ok(()),
        MusubiRegistryAdmissionModeV1::Closed => reject_governance_mutation(
            rejection_reason,
            MusubiGovernanceRejectionReasonV1::PolicyClosed,
            invariant("Musubi registry admission is closed for new records"),
        ),
        MusubiRegistryAdmissionModeV1::Allowlisted => {
            let admitted = if let Some(dataspace) = dataspace {
                policy
                    .allowlisted_dataspaces
                    .binary_search(&dataspace)
                    .is_ok()
            } else {
                world
                    .account_dataspaces(authority)
                    .map_err(|error| invariant(error.reason()))?
                    .iter()
                    .any(|candidate| {
                        policy
                            .allowlisted_dataspaces
                            .binary_search(candidate)
                            .is_ok()
                    })
            };
            if admitted {
                Ok(())
            } else {
                reject_governance_mutation(
                    rejection_reason,
                    MusubiGovernanceRejectionReasonV1::PolicyClosed,
                    invariant("Musubi registry admission requires an allowlisted dataspace"),
                )
            }
        }
    }
}

fn bind_location_reverse_indices(
    existing: Option<&MusubiArchiveLocationV1>,
    location: &MusubiArchiveLocationV1,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let key = location.key();
    match state_transaction
        .world
        .musubi_locations_by_pin
        .get(&location.pin_manifest)
    {
        None => {}
        Some(reference)
            if reference.active
                && reference.location == key
                && existing.is_some_and(|record| record.pin_manifest == location.pin_manifest) => {}
        Some(_) => {
            return Err(invariant(
                "Musubi SoraFS pin manifests cannot be reused by another location or renewal",
            ));
        }
    }
    match state_transaction
        .world
        .musubi_locations_by_replication_order
        .get(&location.replication_order)
    {
        None => {}
        Some(reference)
            if reference.active
                && reference.location == key
                && existing.is_some_and(|record| {
                    record.replication_order == location.replication_order
                }) => {}
        Some(_) => {
            return Err(invariant(
                "Musubi SoraFS replication orders cannot be reused by another location or renewal",
            ));
        }
    }

    if let Some(existing) = existing {
        let old_pin = state_transaction
            .world
            .musubi_locations_by_pin
            .get(&existing.pin_manifest)
            .cloned()
            .ok_or_else(|| invariant("Musubi pin reverse index is inconsistent"))?;
        if !old_pin.active || old_pin.location != key {
            return Err(invariant("Musubi pin reverse index is inconsistent"));
        }
        if existing.pin_manifest != location.pin_manifest {
            state_transaction.world.musubi_locations_by_pin.insert(
                existing.pin_manifest,
                MusubiPinLocationReferenceV1 {
                    active: false,
                    ..old_pin
                },
            );
        }
        let old_order = state_transaction
            .world
            .musubi_locations_by_replication_order
            .get(&existing.replication_order)
            .cloned()
            .ok_or_else(|| invariant("Musubi order reverse index is inconsistent"))?;
        if !old_order.active || old_order.location != key {
            return Err(invariant("Musubi order reverse index is inconsistent"));
        }
        if existing.replication_order != location.replication_order {
            state_transaction
                .world
                .musubi_locations_by_replication_order
                .insert(
                    existing.replication_order,
                    MusubiReplicationOrderLocationReferenceV1 {
                        active: false,
                        ..old_order
                    },
                );
        }
        for provider in &existing.providers {
            state_transaction
                .world
                .musubi_locations_by_provider
                .remove(MusubiProviderLocationKeyV1::new(*provider, key));
        }
    }

    state_transaction.world.musubi_locations_by_pin.insert(
        location.pin_manifest,
        MusubiPinLocationReferenceV1 {
            pin_manifest: location.pin_manifest,
            location: key,
            active: true,
        },
    );
    state_transaction
        .world
        .musubi_locations_by_replication_order
        .insert(
            location.replication_order,
            MusubiReplicationOrderLocationReferenceV1 {
                replication_order: location.replication_order,
                location: key,
                active: true,
            },
        );
    for provider in &location.providers {
        state_transaction
            .world
            .musubi_locations_by_provider
            .insert(MusubiProviderLocationKeyV1::new(*provider, key), ());
    }
    Ok(())
}

fn load_location_provider_attestations(
    archive: &MusubiArchiveRecordV1,
    location: &MusubiArchiveLocationV1,
    world: &impl WorldReadOnly,
) -> Result<Vec<MusubiProviderBundleAttestationRecordV1>, Error> {
    archive
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    location
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if location.archive_id != archive.archive_id {
        return Err(invariant(
            "Musubi archive location does not match its archive directory",
        ));
    }
    let receipt = &archive.staging_receipt.payload.binding;
    let mut verification_lock_digest = None;
    let mut references = Vec::with_capacity(location.providers.len());
    let mut records = Vec::with_capacity(location.providers.len());
    for provider in &location.providers {
        let key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: archive.archive_id,
            replication_order: location.replication_order,
            provider_id: *provider,
        };
        let record = world
            .musubi_provider_bundle_attestations()
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                invariant("Musubi archive location provider attestation record was not found")
            })?;
        record
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        if record.key != key
            || record.registered_at_height < archive.registered_at_height
            || record.registered_at_height >= location.finalized_height
        {
            return Err(invariant(
                "Musubi archive location provider attestation record is not a finalized predecessor",
            ));
        }
        let binding = &record.attestation.payload.binding;
        if binding.chain_id != receipt.chain_id
            || binding.genesis_block_hash != receipt.genesis_block_hash
            || binding.provider_id != *provider
            || binding.replication_order != location.replication_order
            || binding.archive_id != archive.archive_id
            || binding.bundle_digest != archive.commitment.bundle_digest
            || binding.descriptor_digest != archive.commitment.descriptor_digest
            || binding.semantic_release_manifest_digest != receipt.semantic_release_manifest_digest
            || binding.source_tree_digest != archive.commitment.source_tree_digest
        {
            return Err(invariant(
                "Musubi archive location attestation does not match its immutable archive commitments",
            ));
        }
        if verification_lock_digest
            .replace(binding.verification_lock_digest)
            .is_some_and(|digest| digest != binding.verification_lock_digest)
        {
            return Err(invariant(
                "Musubi archive location attestations disagree on the verification lock",
            ));
        }
        record
            .attestation
            .verify(binding)
            .map_err(|error| invariant(error.reason()))?;
        references.push(record.attestation.reference());
        records.push(record);
    }
    let set_digest = musubi_provider_bundle_attestation_set_digest_v1(
        archive.archive_id,
        location.replication_order,
        &references,
    )
    .map_err(|error| invariant(error.reason()))?;
    if set_digest != location.provider_attestation_set_digest {
        return Err(invariant(
            "Musubi archive location provider attestation set digest is inconsistent",
        ));
    }
    Ok(records)
}

fn validate_exact_archive_location_replay(
    archive: &MusubiArchiveRecordV1,
    location: &MusubiArchiveLocationV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    archive
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    location
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    let execution_height = execution_height(state_transaction);
    if location.revision > archive.location_revision
        || archive.registered_at_height > location.finalized_height
        || location.finalized_height > execution_height
    {
        return Err(invariant(
            "Musubi archive location revision or finalized height is inconsistent",
        ));
    }
    let records =
        load_location_provider_attestations(archive, location, state_transaction.world())?;
    if records.len() != location.providers.len() {
        return Err(invariant(
            "Musubi archive location provider attestation registry is inconsistent",
        ));
    }

    let key = location.key();
    let pin_reference = state_transaction
        .world
        .musubi_locations_by_pin
        .get(&location.pin_manifest)
        .ok_or_else(|| invariant("Musubi pin reverse index is inconsistent"))?;
    pin_reference
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if pin_reference.pin_manifest != location.pin_manifest
        || pin_reference.location != key
        || !pin_reference.active
    {
        return Err(invariant("Musubi pin reverse index is inconsistent"));
    }

    let order_reference = state_transaction
        .world
        .musubi_locations_by_replication_order
        .get(&location.replication_order)
        .ok_or_else(|| invariant("Musubi order reverse index is inconsistent"))?;
    order_reference
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if order_reference.replication_order != location.replication_order
        || order_reference.location != key
        || !order_reference.active
    {
        return Err(invariant("Musubi order reverse index is inconsistent"));
    }

    for provider in &location.providers {
        let provider_key = MusubiProviderLocationKeyV1::new(*provider, key);
        provider_key
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        if state_transaction
            .world
            .musubi_locations_by_provider
            .get(&provider_key)
            .is_none()
        {
            return Err(invariant("Musubi provider reverse index is inconsistent"));
        }
    }

    Ok(())
}

fn retire_location_reverse_indices(
    location: &MusubiArchiveLocationV1,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let key = location.key();
    let pin = state_transaction
        .world
        .musubi_locations_by_pin
        .get(&location.pin_manifest)
        .cloned()
        .ok_or_else(|| invariant("Musubi pin reverse index is inconsistent"))?;
    if !pin.active || pin.location != key {
        return Err(invariant("Musubi pin reverse index is inconsistent"));
    }
    state_transaction.world.musubi_locations_by_pin.insert(
        location.pin_manifest,
        MusubiPinLocationReferenceV1 {
            active: false,
            ..pin
        },
    );
    let order = state_transaction
        .world
        .musubi_locations_by_replication_order
        .get(&location.replication_order)
        .cloned()
        .ok_or_else(|| invariant("Musubi order reverse index is inconsistent"))?;
    if !order.active || order.location != key {
        return Err(invariant("Musubi order reverse index is inconsistent"));
    }
    state_transaction
        .world
        .musubi_locations_by_replication_order
        .insert(
            location.replication_order,
            MusubiReplicationOrderLocationReferenceV1 {
                active: false,
                ..order
            },
        );
    for provider in &location.providers {
        state_transaction
            .world
            .musubi_locations_by_provider
            .remove(MusubiProviderLocationKeyV1::new(*provider, key));
    }
    Ok(())
}

fn validate_provider_bundle_attestation(
    archive: &MusubiArchiveRecordV1,
    attestation: &MusubiProviderBundleVerificationAttestationV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    attestation
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    let binding = &attestation.payload.binding;
    let order = state_transaction
        .world
        .replication_orders
        .get(&binding.replication_order)
        .ok_or_else(|| invariant("Musubi provider attestation replication order was not found"))?;
    let completion = order
        .provider_completion(binding.provider_id)
        .ok_or_else(|| invariant("Musubi provider attestation has no finalized completion"))?;
    let current_owner = state_transaction
        .world
        .provider_owners
        .get(&binding.provider_id)
        .ok_or_else(|| invariant("Musubi provider attestation owner is no longer admitted"))?;
    let anchor_index = binding
        .finalized_anchor
        .height
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok());
    let canonical_anchor = anchor_index.and_then(|index| {
        state_transaction
            .block_hashes()
            .get(index)
            .map(|hash| *hash.as_ref())
    });
    if binding.chain_id != *state_transaction.chain_id()
        || binding.genesis_block_hash != genesis_block_hash(state_transaction)?
        || order.order_id != binding.replication_order
        || order.manifest_root_cid != archive.commitment.root_cid
        || current_owner != &completion.completed_by
        || current_owner != &completion.completion_authority.provider_owner
        || binding.completed_by != completion.completed_by
        || binding.completion_authority != completion.completion_authority
        || binding.assignment_revision != completion.assignment_revision
        || binding.completion_epoch != completion.completion_epoch
        || binding.finalized_anchor != completion.finalized_anchor
        || canonical_anchor != Some(binding.finalized_anchor.block_hash)
        || binding.archive_id != archive.archive_id
        || binding.bundle_digest != archive.commitment.bundle_digest
        || binding.descriptor_digest != archive.commitment.descriptor_digest
        || binding.semantic_release_manifest_digest
            != archive
                .staging_receipt
                .payload
                .binding
                .semantic_release_manifest_digest
        || binding.source_tree_digest != archive.commitment.source_tree_digest
    {
        return Err(invariant(
            "Musubi provider attestation does not match the finalized completion or bundle commitment",
        ));
    }
    attestation
        .verify(binding)
        .map_err(|error| invariant(error.reason()))
}

fn validate_sorafs_location(
    archive: &MusubiArchiveRecordV1,
    pin_manifest: &iroha_data_model::sorafs::pin_registry::ManifestDigest,
    replication_order: &iroha_data_model::sorafs::pin_registry::ReplicationOrderId,
    provider_attestation_set_digest: MusubiProviderBundleAttestationSetDigestV1,
    expires_at_epoch: u64,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Vec<iroha_data_model::sorafs::capacity::ProviderId>, Error> {
    archive
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    let commitment = &archive.commitment;
    let pin = state_transaction
        .world
        .pin_manifests
        .get(pin_manifest)
        .ok_or_else(|| invariant("Musubi archive pin manifest was not found"))?;
    if !pin.status.is_active()
        || pin.root_cid != commitment.root_cid
        || pin.chunker != commitment.chunker
        || pin.chunk_digest_sha3_256 != *commitment.chunk_plan_digest.as_bytes()
        || pin.por_root != *commitment.por_root.as_bytes()
        || pin.content_length != commitment.content_length
        || pin.policy.min_replicas < MUSUBI_MIN_HEALTHY_REPLICAS_V1
        || pin.policy.retention_epoch != expires_at_epoch
    {
        return Err(invariant(
            "Musubi archive pin is not finalized or does not match the archive commitment",
        ));
    }
    let order = state_transaction
        .world
        .replication_orders
        .get(replication_order)
        .ok_or_else(|| invariant("Musubi archive replication order was not found"))?;
    if order.manifest_digest != *pin_manifest
        || order.manifest_root_cid != commitment.root_cid
        || order.deadline_epoch > expires_at_epoch
        || !matches!(order.status, ReplicationOrderStatus::Completed(_))
    {
        return Err(invariant(
            "Musubi replication order is not finalized for the requested archive pin",
        ));
    }
    let mut providers = order
        .provider_completions
        .iter()
        .map(|completion| completion.provider_id)
        .collect::<Vec<_>>();
    providers.sort();
    if providers.is_empty()
        || providers.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1
        || providers.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invariant(
            "Musubi replication order has an invalid provider completion set",
        ));
    }
    let mut references = Vec::with_capacity(providers.len());
    let mut verification_lock_digest = None;
    let add_height = execution_height(state_transaction);
    for provider in &providers {
        let key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: archive.archive_id,
            replication_order: *replication_order,
            provider_id: *provider,
        };
        let record = state_transaction
            .world
            .musubi_provider_bundle_attestations
            .get(&key)
            .ok_or_else(|| {
                invariant("Musubi finalized completion has no registered provider attestation")
            })?;
        record
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        if record.key != key || record.registered_at_height >= add_height {
            return Err(invariant(
                "Musubi provider attestation must be finalized before archive location admission",
            ));
        }
        validate_provider_bundle_attestation(archive, &record.attestation, state_transaction)?;
        let binding = &record.attestation.payload.binding;
        if verification_lock_digest
            .replace(binding.verification_lock_digest)
            .is_some_and(|digest| digest != binding.verification_lock_digest)
        {
            return Err(invariant(
                "Musubi provider attestations disagree on the verification lock",
            ));
        }
        references.push(record.attestation.reference());
    }
    let computed_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
        archive.archive_id,
        *replication_order,
        &references,
    )
    .map_err(|error| invariant(error.reason()))?;
    if computed_set_digest != provider_attestation_set_digest {
        return Err(invariant(
            "Musubi provider attestation set digest does not cover the finalized completion set",
        ));
    }
    Ok(providers)
}

fn validate_seed_ingress_receipt(
    commitment: &MusubiArchiveCommitmentV1,
    receipt: &MusubiSeedIngressReceiptV1,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let binding = &receipt.payload.binding;
    let archive_id = commitment.archive_id();
    if binding.chain_id != *state_transaction.chain_id()
        || binding.genesis_block_hash != genesis_block_hash(state_transaction)?
        || binding.publisher != *authority
        || binding.archive_id != archive_id
        || binding.car_body_digest != commitment.car_digest
        || binding.car_body_length != commitment.car_size
    {
        return Err(invariant(
            "Musubi seed-ingress receipt does not match the chain, publisher, or archive body",
        ));
    }
    let admitted_owner = state_transaction
        .world
        .provider_owners
        .get(&binding.seed_provider)
        .ok_or_else(|| invariant("Musubi seed-ingress provider is not admitted"))?;
    if admitted_owner != &binding.ingress_broker {
        return Err(invariant(
            "Musubi seed-ingress receipt was not signed by the admitted provider owner",
        ));
    }
    receipt
        .verify(binding, execution_time_ms(state_transaction)?)
        .map_err(|error| invariant(error.reason()))
}

fn validate_publication_archive_evidence(
    publication: &MusubiPublicationV1,
    archive: &MusubiArchiveRecordV1,
    authority: &AccountId,
    world: &impl WorldReadOnly,
) -> Result<(), Error> {
    let receipt_binding = &archive.staging_receipt.payload.binding;
    let semantic_digest = publication.manifest.semantic_digest();
    if receipt_binding.publisher != *authority
        || receipt_binding.semantic_release_manifest_digest != semantic_digest
    {
        return Err(invariant(
            "Musubi release does not match the authenticated seed-ingress receipt",
        ));
    }

    let mut verified_providers = BTreeSet::new();
    for location_id in &archive.location_ids {
        let key = MusubiArchiveLocationKeyV1::new(archive.archive_id, *location_id);
        let location = world
            .musubi_archive_locations()
            .get(&key)
            .ok_or_else(|| invariant("Musubi archive location directory is inconsistent"))?;
        let records = load_location_provider_attestations(archive, location, world)?;
        for record in records {
            let binding = &record.attestation.payload.binding;
            if binding.semantic_release_manifest_digest != semantic_digest
                || binding.verification_lock_digest != publication.manifest.verification_lock_digest
                || binding.bundle_digest != archive.commitment.bundle_digest
                || binding.descriptor_digest != archive.commitment.descriptor_digest
                || binding.source_tree_digest != archive.commitment.source_tree_digest
            {
                return Err(invariant(
                    "Musubi provider bundle attestation does not match the release bundle",
                ));
            }
            verified_providers.insert(binding.provider_id);
        }
    }
    if verified_providers.len() < usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1) {
        return Err(invariant(
            "Musubi release lacks three distinct parsed-bundle provider attestations",
        ));
    }
    Ok(())
}

fn validate_publication_snapshot(
    publication: &MusubiPublicationV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let snapshot = &publication.resolution.snapshot;
    let current_height = u64::try_from(state_transaction.block_hashes().len())
        .map_err(|_| invariant("Musubi finalized height overflows u64"))?;
    let snapshot_index = snapshot
        .finalized_height
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok());
    let finalized_hash = snapshot_index.and_then(|index| {
        state_transaction
            .block_hashes()
            .get(index)
            .map(|hash| *hash.as_ref())
    });
    let current_revision = state_transaction
        .world
        .musubi_resolver_index_revision
        .get()
        .get();
    validate_publication_snapshot_anchor(snapshot, current_height, finalized_hash, current_revision)
}

fn validate_publication_snapshot_anchor(
    snapshot: &MusubiRegistrySnapshotV1,
    current_height: u64,
    finalized_hash: Option<[u8; 32]>,
    current_revision: u64,
) -> Result<(), Error> {
    snapshot
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    let finalized_hash = finalized_hash
        .ok_or_else(|| invariant("Musubi publication requires a canonical finalized snapshot"))?;
    if snapshot.finalized_height > current_height
        || snapshot.finalized_block_hash != finalized_hash
        || snapshot.index_revision > current_revision
    {
        return Err(invariant(
            "Musubi publication proof snapshot is not a canonical finalized registry ancestor",
        ));
    }
    Ok(())
}

fn validate_resolution_proof(
    publication: &MusubiPublicationV1,
    world: &impl WorldReadOnly,
) -> Result<(), Error> {
    let nodes = publication
        .resolution
        .lock
        .nodes
        .iter()
        .map(|node| (&node.release, node))
        .collect::<BTreeMap<_, _>>();
    for node in &publication.resolution.lock.nodes {
        let row = world
            .musubi_resolver_index()
            .get(&node.release)
            .ok_or_else(|| {
                invariant(format!(
                    "Musubi proof release '{}' is absent from the resolver index",
                    node.release
                ))
            })?;
        if !row.selection.fresh_selectable()
            || row.release_digest != node.release_digest
            || row.archive_id != node.archive_id
            || row.source_digest != node.source_digest
            || row.interface_digest != node.interface_digest
            || row.abi != node.abi
        {
            return Err(invariant(format!(
                "Musubi proof release '{}' does not match a fresh-selectable resolver row",
                node.release
            )));
        }
        if row.dependencies.len() != node.dependencies.len()
            || row
                .dependencies
                .iter()
                .zip(&node.dependencies)
                .any(|(requirement, exact)| {
                    requirement.alias != exact.alias
                        || requirement.package != exact.package
                        || requirement.requirement != exact.requirement
                        || exact.kind != MusubiDependencyKindV1::Normal
                })
        {
            return Err(invariant(format!(
                "Musubi proof release '{}' dependency edges do not match the published row",
                node.release
            )));
        }
    }
    let mut reachable = BTreeSet::new();
    let mut pending = publication
        .resolution
        .lock
        .root_dependencies
        .iter()
        .map(|edge| edge.selected.clone())
        .collect::<Vec<_>>();
    while let Some(release) = pending.pop() {
        if !reachable.insert(release.clone()) {
            continue;
        }
        let node = nodes
            .get(&release)
            .ok_or_else(|| invariant("Musubi proof references an absent exact node"))?;
        pending.extend(
            node.dependencies
                .iter()
                .filter(|edge| edge.kind == MusubiDependencyKindV1::Normal)
                .map(|edge| edge.selected.clone()),
        );
    }
    if reachable.len() != nodes.len() {
        return Err(invariant(
            "Musubi publication proof contains unreachable exact nodes",
        ));
    }
    Ok(())
}

fn plan_archive_reverse_reference(
    archive_id: ArchiveId,
    release: MusubiReleaseIdV1,
    world: &impl WorldReadOnly,
) -> Result<MusubiArchiveReverseReferencesV1, Error> {
    let mut references = world
        .musubi_archive_reverse_references()
        .get(&archive_id)
        .cloned()
        .ok_or_else(|| {
            invariant("Musubi archive is missing its exact reverse-reference projection")
        })?;
    references
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if references.archive_id != archive_id {
        return Err(invariant(
            "Musubi archive reverse-reference projection has the wrong archive identity",
        ));
    }
    references.releases.push(release);
    references.releases.sort();
    references.releases.dedup();
    references
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    Ok(references)
}

pub(crate) fn current_location_providers(
    location: &MusubiArchiveLocationV1,
    world: &impl WorldReadOnly,
) -> Option<Vec<iroha_data_model::sorafs::capacity::ProviderId>> {
    if location.state == MusubiArchiveLocationStateV1::Retired || location.validate().is_err() {
        return None;
    }
    let key = location.key();
    if !world
        .musubi_locations_by_pin()
        .get(&location.pin_manifest)
        .is_some_and(|reference| reference.active && reference.location == key)
        || !world
            .musubi_locations_by_replication_order()
            .get(&location.replication_order)
            .is_some_and(|reference| reference.active && reference.location == key)
    {
        return None;
    }
    let archive = world.musubi_archives().get(&location.archive_id)?;
    archive.validate().ok()?;
    let pin = world.pin_manifests().get(&location.pin_manifest)?;
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
        .replication_orders()
        .get(&location.replication_order)?;
    if order.manifest_digest != location.pin_manifest
        || order.manifest_root_cid != archive.commitment.root_cid
        || !matches!(order.status, ReplicationOrderStatus::Completed(_))
    {
        return None;
    }
    let mut completion_providers = order
        .provider_completions
        .iter()
        .map(|completion| completion.provider_id)
        .collect::<Vec<_>>();
    completion_providers.sort();
    if completion_providers != location.providers {
        return None;
    }
    let records = load_location_provider_attestations(archive, location, world).ok()?;
    let providers = location
        .providers
        .iter()
        .zip(&records)
        .filter_map(|(provider, record)| {
            let reverse_key = MusubiProviderLocationKeyV1::new(*provider, key);
            if world
                .musubi_locations_by_provider()
                .get(&reverse_key)
                .is_none()
            {
                return None;
            }
            let owner = world.provider_owners().get(provider)?;
            let completion = order.provider_completion(*provider)?;
            let binding = &record.attestation.payload.binding;
            (completion.completed_by == *owner
                && completion.completion_authority.provider_owner == *owner
                && binding.provider_id == *provider
                && binding.completed_by == completion.completed_by
                && binding.completion_authority == completion.completion_authority
                && binding.assignment_revision == completion.assignment_revision
                && binding.completion_epoch == completion.completion_epoch
                && binding.finalized_anchor == completion.finalized_anchor)
                .then_some(*provider)
        })
        .collect::<Vec<_>>();
    Some(providers)
}

/// Prevent an explicit lifecycle change from removing the last quorum-healthy
/// location of an active or yanked release.
pub(crate) fn ensure_locations_may_be_invalidated(
    locations: &[MusubiArchiveLocationKeyV1],
    world: &impl WorldReadOnly,
) -> Result<(), Error> {
    let excluded = locations.iter().copied().collect::<BTreeSet<_>>();
    let archives = locations
        .iter()
        .map(|location| location.archive_id)
        .collect::<BTreeSet<_>>();
    for archive_id in archives {
        if !archive_has_protected_release(archive_id, world) {
            continue;
        }
        let archive = world
            .musubi_archives()
            .get(&archive_id)
            .ok_or_else(|| archive_not_found(archive_id))?;
        let mut remaining_providers = BTreeSet::new();
        for location_id in &archive.location_ids {
            let key = MusubiArchiveLocationKeyV1::new(archive_id, *location_id);
            if excluded.contains(&key) {
                continue;
            }
            if let Some(providers) = world
                .musubi_archive_locations()
                .get(&key)
                .and_then(|location| current_location_providers(location, world))
            {
                remaining_providers.extend(providers);
            }
        }
        if !provider_count_is_healthy(remaining_providers.len()) {
            return Err(invariant(
                "Musubi active or yanked releases must retain a quorum-healthy archive location",
            ));
        }
    }
    Ok(())
}

/// Prevent provider removal from making a protected archive entirely
/// unfetchable. Falling below fresh-selection quorum remains allowed.
pub(crate) fn ensure_provider_may_be_removed(
    provider: iroha_data_model::sorafs::capacity::ProviderId,
    locations: &[MusubiArchiveLocationKeyV1],
    world: &impl WorldReadOnly,
) -> Result<(), Error> {
    let archives = locations
        .iter()
        .map(|location| location.archive_id)
        .collect::<BTreeSet<_>>();
    for archive_id in archives {
        if !archive_has_protected_release(archive_id, world) {
            continue;
        }
        let archive = world
            .musubi_archives()
            .get(&archive_id)
            .ok_or_else(|| archive_not_found(archive_id))?;
        let has_remaining = archive.location_ids.iter().any(|location_id| {
            let key = MusubiArchiveLocationKeyV1::new(archive_id, *location_id);
            world
                .musubi_archive_locations()
                .get(&key)
                .and_then(|location| current_location_providers(location, world))
                .is_some_and(|providers| providers.into_iter().any(|item| item != provider))
        });
        if !has_remaining {
            return Err(invariant(
                "SoraFS provider removal would eliminate the last fetchable location of an active or yanked Musubi release",
            ));
        }
    }
    Ok(())
}

const fn provider_count_is_healthy(provider_count: usize) -> bool {
    provider_count >= MUSUBI_MIN_HEALTHY_REPLICAS_V1 as usize
}

/// Recompute location and universal resolver projections after an underlying
/// SoraFS lifecycle change. Every lookup is exact or bounded by archive-local
/// directories populated at registration.
pub(crate) fn refresh_musubi_locations(
    locations: &[MusubiArchiveLocationKeyV1],
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let locations = locations.iter().copied().collect::<BTreeSet<_>>();
    let mut archives = BTreeSet::new();
    for key in locations {
        let Some(mut location) = state_transaction
            .world
            .musubi_archive_locations
            .get(&key)
            .cloned()
        else {
            continue;
        };
        if location.state == MusubiArchiveLocationStateV1::Retired {
            continue;
        }
        archives.insert(key.archive_id);
        let healthy_replicas = current_location_providers(&location, state_transaction.world())
            .map_or(0, |providers| providers.len());
        let next_state = if healthy_replicas >= usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1) {
            MusubiArchiveLocationStateV1::Healthy
        } else {
            MusubiArchiveLocationStateV1::Degraded
        };
        if location.state == next_state {
            continue;
        }
        let mut archive = state_transaction
            .world
            .musubi_archives
            .get(&key.archive_id)
            .cloned()
            .ok_or_else(|| archive_not_found(key.archive_id))?;
        let revision = next_revision(archive.location_revision, "archive location lifecycle")?;
        archive.location_revision = revision;
        location.revision = revision;
        location.finalized_height = execution_height(state_transaction);
        location.state = next_state;
        let event = MusubiArchiveLocationEventV1 {
            location: key,
            pin_manifest: location.pin_manifest,
            replication_order: location.replication_order,
            provider_attestation_set_digest: location.provider_attestation_set_digest,
            provider_count: u8::try_from(location.providers.len())
                .map_err(|_| invariant("Musubi archive location provider count overflows u8"))?,
            transition: MusubiArchiveLocationTransitionV1::EvidenceRefreshed,
            state: location.state,
            revision: location.revision,
            finalized_height: location.finalized_height,
        };
        state_transaction
            .world
            .musubi_archive_locations
            .insert(key, location);
        state_transaction
            .world
            .musubi_archives
            .insert(key.archive_id, archive);
        emit_musubi_event(
            MusubiEvent::ArchiveLocationChanged(event),
            state_transaction,
        );
    }
    for archive_id in archives {
        refresh_archive_availability(archive_id, state_transaction)?;
    }
    Ok(())
}

fn refresh_archive_availability(
    archive_id: ArchiveId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let previous = state_transaction
        .world
        .musubi_archive_availability
        .get(&archive_id)
        .copied()
        .ok_or_else(|| {
            invariant("Musubi archive is missing its authoritative availability projection")
        })?;
    let references =
        exact_archive_projection_references(archive_id, previous, state_transaction.world())?;
    let archive = state_transaction
        .world
        .musubi_archives
        .get(&archive_id)
        .cloned()
        .ok_or_else(|| archive_not_found(archive_id))?;
    archive
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if archive.archive_id != archive_id {
        return Err(invariant(
            "Musubi archive record has the wrong embedded archive identity",
        ));
    }
    let locations = archive
        .location_ids
        .iter()
        .map(|location_id| {
            let key = MusubiArchiveLocationKeyV1::new(archive_id, *location_id);
            let location = state_transaction
                .world
                .musubi_archive_locations
                .get(&key)
                .cloned()
                .ok_or_else(|| invariant("Musubi archive location directory is inconsistent"))?;
            if location.key() != key {
                return Err(invariant(
                    "Musubi archive location record has the wrong embedded identity",
                ));
            }
            location
                .validate()
                .map_err(|error| invariant(error.reason()))?;
            if location.revision > archive.location_revision {
                return Err(invariant(
                    "Musubi archive location revision exceeds its archive revision",
                ));
            }
            Ok(location)
        })
        .collect::<Result<Vec<_>, Error>>()?;
    if locations
        .iter()
        .any(|record| record.state == MusubiArchiveLocationStateV1::Retired)
    {
        return Err(invariant(
            "Musubi active archive location directory contains a retired record",
        ));
    }
    let current = locations
        .iter()
        .filter_map(|location| {
            current_location_providers(location, state_transaction.world())
                .map(|providers| (location, providers))
        })
        .collect::<Vec<_>>();
    let mut providers = current
        .iter()
        .flat_map(|(_, providers)| providers.iter().copied())
        .collect::<Vec<_>>();
    providers.sort();
    providers.dedup();
    let active_locations = u8::try_from(current.len())
        .map_err(|_| invariant("Musubi active archive-location count overflows u8"))?;
    let healthy_replicas = u16::try_from(providers.len())
        .map_err(|_| invariant("Musubi healthy replica count overflows u16"))?;
    let availability = if healthy_replicas >= MUSUBI_MIN_HEALTHY_REPLICAS_V1 {
        MusubiStorageAvailabilityV1::Selectable
    } else if active_locations > 0 && healthy_replicas > 0 {
        MusubiStorageAvailabilityV1::BelowQuorum
    } else {
        MusubiStorageAvailabilityV1::Unavailable
    };
    if previous.availability == availability
        && previous.healthy_replicas == healthy_replicas
        && previous.active_locations == active_locations
    {
        return Ok(());
    }
    let release_count = u64::try_from(references.len())
        .map_err(|_| invariant("Musubi archive reverse-reference count overflows u64"))?;
    let replication_shortfall_update = plan_replication_shortfall_transition(
        *state_transaction
            .world
            .musubi_replication_shortfall_releases
            .get(),
        previous.availability,
        availability,
        release_count,
    )?;
    let index_revision = bump_resolver_index_revision(state_transaction)?;
    let projection = MusubiArchiveAvailabilityV1 {
        archive_id,
        availability,
        healthy_replicas,
        active_locations,
        finalized_height: execution_height(state_transaction),
        finalized_block_hash: execution_hash(state_transaction),
        index_revision,
    };
    projection
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    state_transaction
        .world
        .musubi_archive_availability
        .insert(archive_id, projection);
    if let Some(replication_shortfall_releases) = replication_shortfall_update {
        *state_transaction
            .world
            .musubi_replication_shortfall_releases
            .get_mut() = replication_shortfall_releases;
    }
    let mut packages = BTreeSet::new();
    for release in references {
        let mut row = state_transaction
            .world
            .musubi_resolver_index
            .get(&release)
            .cloned()
            .ok_or_else(|| {
                invariant("Musubi archive reverse reference is missing its exact resolver row")
            })?;
        row.selection.storage = projection;
        row.index_revision = index_revision;
        packages.insert(release.package.clone());
        state_transaction
            .world
            .musubi_resolver_index
            .insert(release, row);
    }
    for package in packages {
        refresh_directory_for_package(&package, index_revision, state_transaction)?;
    }
    emit_musubi_event(
        MusubiEvent::ArchiveAvailabilityChanged(projection),
        state_transaction,
    );
    Ok(())
}

fn exact_archive_projection_references(
    archive_id: ArchiveId,
    availability: MusubiArchiveAvailabilityV1,
    world: &impl WorldReadOnly,
) -> Result<Vec<MusubiReleaseIdV1>, Error> {
    let archive = world
        .musubi_archives()
        .get(&archive_id)
        .ok_or_else(|| archive_not_found(archive_id))?;
    archive
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if archive.archive_id != archive_id {
        return Err(invariant(
            "Musubi archive record has the wrong embedded archive identity",
        ));
    }
    availability
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if availability.archive_id != archive_id {
        return Err(invariant(
            "Musubi archive availability projection has the wrong archive identity",
        ));
    }
    let references = world
        .musubi_archive_reverse_references()
        .get(&archive_id)
        .ok_or_else(|| {
            invariant("Musubi archive is missing its exact reverse-reference projection")
        })?;
    references
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    if references.archive_id != archive_id {
        return Err(invariant(
            "Musubi archive reverse-reference projection has the wrong archive identity",
        ));
    }

    let mut packages = BTreeSet::new();
    for release_id in &references.releases {
        let release = world.musubi_releases().get(release_id).ok_or_else(|| {
            invariant("Musubi archive reverse reference names a missing authoritative release")
        })?;
        release
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        if release.manifest.release != *release_id || release.manifest.archive_id != archive_id {
            return Err(invariant(
                "Musubi archive reverse reference disagrees with its authoritative release",
            ));
        }
        let row = world
            .musubi_resolver_index()
            .get(release_id)
            .ok_or_else(|| {
                invariant("Musubi archive reverse reference is missing its exact resolver row")
            })?;
        row.validate().map_err(|error| invariant(error.reason()))?;
        if row.release != *release_id
            || row.release_digest != release.release_digest
            || row.archive_id != archive_id
            || row.source_digest != archive.commitment.source_tree_digest
            || row.interface_digest != release.manifest.interface_digest
            || row.abi != release.manifest.abi
            || row.dependencies.as_slice() != release.manifest.dependencies.as_slice()
            || row.selection.yank != release.yank
            || row.selection.storage != availability
            || row.selection.governance != release.artifact_governance
        {
            return Err(invariant(
                "Musubi resolver row diverges from its authoritative archive/release projections",
            ));
        }
        packages.insert(release_id.package.clone());
    }
    for package_id in packages {
        let package = world.musubi_packages().get(&package_id).ok_or_else(|| {
            invariant("Musubi archive projection references a missing package record")
        })?;
        package
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        if package.package != package_id {
            return Err(invariant(
                "Musubi archive projection package identity is inconsistent",
            ));
        }
    }
    Ok(references.releases.clone())
}

fn refresh_directory_for_package(
    package: &MusubiPackageIdV1,
    index_revision: u64,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let package_record = state_transaction
        .world
        .musubi_packages
        .get(package)
        .cloned()
        .ok_or_else(|| package_not_found(package))?;
    let latest_selectable = state_transaction
        .world
        .musubi_resolver_index
        .range(package_release_start(package)..)
        .take_while(|(release, _)| release.package == *package)
        .filter(|(_, row)| row.selection.fresh_selectable())
        .map(|(release, _)| release.version.clone())
        .max();
    let entry = plan_package_directory_entry(&package_record, latest_selectable, index_revision)?;
    state_transaction
        .world
        .musubi_public_directory
        .insert(entry.selector.clone(), entry);
    Ok(())
}

fn plan_package_directory_entry(
    package: &MusubiPackageRecordV1,
    latest_selectable: Option<MusubiVersionV1>,
    index_revision: u64,
) -> Result<MusubiOrderedPackageEntryV1, Error> {
    let selector = MusubiPackageSelectorV1 {
        namespace: package.claimed_namespace.clone(),
        name: package.package.name.clone(),
    };
    let entry = MusubiOrderedPackageEntryV1 {
        selector,
        package: package.package.clone(),
        latest_selectable,
        metadata_revision: package.revisions.metadata,
        index_revision,
    };
    entry
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    Ok(entry)
}

fn archive_has_protected_release(archive_id: ArchiveId, world: &impl WorldReadOnly) -> bool {
    world
        .musubi_archive_reverse_references()
        .get(&archive_id)
        .into_iter()
        .flat_map(|references| &references.releases)
        .filter_map(|release| world.musubi_releases().get(release))
        .any(|record| {
            matches!(
                record.artifact_governance,
                MusubiArtifactGovernanceStateV1::Available
            )
        })
}

fn package_has_active_release(package: &MusubiPackageIdV1, world: &impl WorldReadOnly) -> bool {
    world
        .musubi_resolver_index()
        .range(package_release_start(package)..)
        .take_while(|(release, _)| release.package == *package)
        .any(|(_, row)| row.selection.fresh_selectable())
}

fn package_release_start(package: &MusubiPackageIdV1) -> MusubiReleaseIdV1 {
    MusubiReleaseIdV1::new(
        package.clone(),
        MusubiVersionV1::new(0, 0, 0, vec![MusubiPrereleaseIdentifierV1::Numeric(0)])
            .expect("the static minimum Musubi version is valid"),
    )
}

fn package_release_page_start(
    package: &MusubiPackageIdV1,
    page: &MusubiPageRequestV1,
) -> Result<MusubiReleaseIdV1, MusubiQueryExecutionErrorV1> {
    if let Some(cursor) = &page.cursor {
        let version = cursor.last_key.parse::<MusubiVersionV1>().map_err(|_| {
            MusubiQueryExecutionErrorV1::cursor(MusubiCursorFailureV1::LastKeyStale)
        })?;
        Ok(MusubiReleaseIdV1::new(package.clone(), version))
    } else {
        Ok(package_release_start(package))
    }
}

fn verify_parliament_decision(
    decision: &MusubiGovernanceDecisionV1,
    action: &MusubiParliamentActionV1,
    state_transaction: &StateTransaction<'_, '_>,
    rejection_reason: &mut MusubiGovernanceRejectionReasonV1,
) -> Result<(), Error> {
    decision.validate().map_err(|error| {
        *rejection_reason = MusubiGovernanceRejectionReasonV1::InvalidDecision;
        invalid_parameter(error.reason())
    })?;
    action.validate().map_err(|error| {
        *rejection_reason = MusubiGovernanceRejectionReasonV1::InvalidDecision;
        invalid_parameter(error.reason())
    })?;
    if decision.action_digest != action.action_digest() {
        return reject_governance_mutation(
            rejection_reason,
            MusubiGovernanceRejectionReasonV1::InvalidDecision,
            invariant("Musubi Parliament decision does not bind the exact requested action"),
        );
    }
    if state_transaction
        .world
        .musubi_governance_decisions
        .get(&decision.decision_id)
        .is_some()
    {
        return reject_governance_mutation(
            rejection_reason,
            MusubiGovernanceRejectionReasonV1::Replay,
            invariant("Musubi Parliament decision was already consumed"),
        );
    }
    let proposal = state_transaction
        .world
        .governance_proposals
        .get(&decision.decision_id)
        .ok_or_else(|| {
            *rejection_reason = MusubiGovernanceRejectionReasonV1::InvalidDecision;
            invariant("Musubi Parliament decision has no governance proposal")
        })?;
    if proposal.kind.fingerprint() != decision.decision_id {
        return reject_governance_mutation(
            rejection_reason,
            MusubiGovernanceRejectionReasonV1::InvalidDecision,
            invariant(
                "Musubi Parliament proposal storage key differs from its exact typed fingerprint",
            ),
        );
    }
    if proposal.status != GovernanceProposalStatus::Enacted
        || proposal.enacted_at_height != Some(decision.enacted_at_height)
    {
        return reject_governance_mutation(
            rejection_reason,
            MusubiGovernanceRejectionReasonV1::InvalidDecision,
            invariant("Musubi Parliament proposal is not enacted at the claimed height"),
        );
    }
    match &proposal.kind {
        ProposalKind::MusubiRegistryGovernance(enacted) if enacted == action => {}
        _ => {
            return reject_governance_mutation(
                rejection_reason,
                MusubiGovernanceRejectionReasonV1::InvalidDecision,
                invariant("Musubi Parliament proposal payload does not match the requested action"),
            );
        }
    }
    let minimum = decision
        .enacted_at_height
        .checked_add(state_transaction.gov.min_enactment_delay)
        .ok_or_else(|| {
            *rejection_reason = MusubiGovernanceRejectionReasonV1::InvalidDecision;
            invariant("Musubi Parliament delay overflows block height")
        })?;
    if decision.execute_after_height < minimum
        || execution_height(state_transaction) < decision.execute_after_height
    {
        return reject_governance_mutation(
            rejection_reason,
            MusubiGovernanceRejectionReasonV1::InvalidDecision,
            invariant("Musubi Parliament decision has not satisfied the enactment delay"),
        );
    }
    Ok(())
}

fn consume_parliament_decision(
    decision: MusubiGovernanceDecisionV1,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let consumption = MusubiGovernanceDecisionConsumptionV1 {
        decision,
        minimum_enactment_delay: state_transaction.gov.min_enactment_delay,
        consumed_at_height: execution_height(state_transaction),
    };
    consumption
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    state_transaction
        .world
        .musubi_governance_decisions
        .insert(decision.decision_id, consumption);
    Ok(())
}

fn bump_resolver_index_revision(
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<u64, Error> {
    let next = plan_resolver_index_revision(state_transaction)?;
    *state_transaction
        .world
        .musubi_resolver_index_revision
        .get_mut() = next;
    Ok(next.get())
}

fn plan_replication_shortfall_transition(
    current: u64,
    previous: MusubiStorageAvailabilityV1,
    next: MusubiStorageAvailabilityV1,
    release_count: u64,
) -> Result<Option<u64>, Error> {
    let previous_selectable = previous == MusubiStorageAvailabilityV1::Selectable;
    let next_selectable = next == MusubiStorageAvailabilityV1::Selectable;
    if previous_selectable == next_selectable || release_count == 0 {
        return Ok(None);
    }
    if next_selectable {
        current
            .checked_sub(release_count)
            .map(Some)
            .ok_or_else(|| invariant("Musubi replication-shortfall release count would underflow"))
    } else {
        current
            .checked_add(release_count)
            .map(Some)
            .ok_or_else(|| invariant("Musubi replication-shortfall release count would overflow"))
    }
}

fn plan_resolver_index_revision(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<MusubiResolverIndexRevisionV1, Error> {
    (*state_transaction.world.musubi_resolver_index_revision.get())
        .checked_next()
        .ok_or_else(|| invariant("Musubi resolver-index revision overflow"))
}

fn execution_height(state_transaction: &StateTransaction<'_, '_>) -> u64 {
    state_transaction._curr_block.height().get()
}

fn execution_hash(state_transaction: &StateTransaction<'_, '_>) -> [u8; 32] {
    *state_transaction._curr_block.hash().as_ref()
}

fn execution_time_ms(state_transaction: &StateTransaction<'_, '_>) -> Result<u64, Error> {
    u64::try_from(state_transaction._curr_block.creation_time().as_millis())
        .map_err(|_| invariant("Musubi block creation time overflows u64 milliseconds"))
}

fn genesis_block_hash(state_transaction: &StateTransaction<'_, '_>) -> Result<[u8; 32], Error> {
    state_transaction
        .block_hashes()
        .first()
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| invariant("Musubi publication requires a committed genesis block"))
}

fn ensure_revision(label: &str, actual: u64, expected: u64) -> Result<(), Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(stale_revision(label, actual, expected))
    }
}

fn next_revision(revision: u64, label: &str) -> Result<u64, Error> {
    revision
        .checked_add(1)
        .ok_or_else(|| invariant(format!("Musubi {label} revision overflow")))
}

fn stale_revision(label: &str, actual: u64, expected: u64) -> Error {
    invariant(format!(
        "stale Musubi {label} revision: expected {expected}, current {actual}"
    ))
}

fn package_not_found(package: &MusubiPackageIdV1) -> Error {
    invariant(format!("Musubi package '{package}' was not found"))
}

fn release_not_found(release: &MusubiReleaseIdV1) -> Error {
    invariant(format!("Musubi release '{release}' was not found"))
}

fn archive_not_found(archive: ArchiveId) -> Error {
    invariant(format!(
        "Musubi archive '{}' was not found",
        digest_label(archive.as_bytes())
    ))
}

fn digest_label(bytes: &[u8; 32]) -> String {
    hex::encode(bytes)
}

fn invalid_parameter(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}

fn invariant(message: impl Into<String>) -> Error {
    Error::InvariantViolation(message.into().into())
}

fn emit_musubi_event(event: MusubiEvent, state_transaction: &mut StateTransaction<'_, '_>) {
    state_transaction
        .world
        .emit_events(Some(DataEvent::Musubi(event)));
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use mv::cell::Cell;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{GovernancePipeline, GovernanceProposalRecord, State, World},
    };

    const GOVERNANCE_EXECUTION_HEIGHT: u64 = 42;

    fn location_fixture(
        archive_byte: u8,
        pin: iroha_data_model::sorafs::pin_registry::ManifestDigest,
        order: iroha_data_model::sorafs::pin_registry::ReplicationOrderId,
    ) -> MusubiArchiveLocationV1 {
        MusubiArchiveLocationV1 {
            location_id: MusubiArchiveLocationIdV1::new([archive_byte; 32]),
            archive_id: ArchiveId::new([archive_byte; 32]),
            pin_manifest: pin,
            replication_order: order,
            providers: Vec::new(),
            provider_attestation_set_digest: MusubiProviderBundleAttestationSetDigestV1::new(
                [archive_byte.wrapping_add(1); 32],
            ),
            renew_after_epoch: 1,
            expires_at_epoch: 2,
            finalized_height: 1,
            revision: 1,
            state: MusubiArchiveLocationStateV1::Healthy,
        }
    }

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives an account");
        AccountId::new(keypair.public_key().clone())
    }

    #[cfg(feature = "telemetry")]
    fn governance_rejection_counts(
        metrics: &crate::telemetry::Metrics,
        action: &str,
        reason: &str,
    ) -> (u64, u64) {
        let exposition = metrics.try_to_string().expect("encode metrics");
        let action_label = format!("action=\"{action}\"");
        let reason_label = format!("reason=\"{reason}\"");
        exposition
            .lines()
            .filter(|line| line.starts_with("musubi_governance_rejections_total{"))
            .fold((0_u64, 0_u64), |(total, exact), line| {
                let (labels, value) = line
                    .rsplit_once(' ')
                    .expect("Prometheus counter sample has a value");
                let value = value.parse::<u64>().expect("counter sample is an integer");
                let exact = if labels.contains(&action_label) && labels.contains(&reason_label) {
                    exact + value
                } else {
                    exact
                };
                (total + value, exact)
            })
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn governance_rejections_are_counted_once_at_the_authoritative_isi_boundary() {
        use std::sync::Arc;

        let metrics = Arc::new(crate::telemetry::Metrics::default());
        let telemetry = crate::telemetry::StateTelemetry::new(Arc::clone(&metrics), true);
        let state = State::with_telemetry(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            telemetry,
        );
        let before_unauthorized = governance_rejection_counts(&metrics, "remove", "unauthorized");
        let before_stale = governance_rejection_counts(&metrics, "remove", "stale_revision");
        let before_last_owner = governance_rejection_counts(&metrics, "remove", "last_owner");

        {
            let header = iroha_data_model::block::BlockHeader::new(
                std::num::NonZeroU64::new(1).expect("nonzero block height"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            let owner = account(11);
            let stranger = account(12);
            let package = package("telemetry-last-owner");
            seed_package_owner(&package, &owner, 1, &mut transaction);
            let remove = |expected_governance_revision| RemoveMusubiPackageMaintainerV1 {
                package: package.clone(),
                account: owner.clone(),
                expected_governance_revision,
            };

            remove(1)
                .execute(&stranger, &mut transaction)
                .expect_err("a non-owner must be rejected");
            remove(2)
                .execute(&owner, &mut transaction)
                .expect_err("a stale governance revision must be rejected");
            let error = remove(1)
                .execute(&owner, &mut transaction)
                .expect_err("the sole owner cannot be removed");
            assert!(error.to_string().contains("retain its last owner"));
        }

        let after_unauthorized = governance_rejection_counts(&metrics, "remove", "unauthorized");
        let after_stale = governance_rejection_counts(&metrics, "remove", "stale_revision");
        let after_last_owner = governance_rejection_counts(&metrics, "remove", "last_owner");
        assert_eq!(after_unauthorized.1, before_unauthorized.1 + 1);
        assert_eq!(after_stale.1, before_stale.1 + 1);
        assert_eq!(after_last_owner.1, before_last_owner.1 + 1);
        assert_eq!(after_last_owner.0, before_last_owner.0 + 3);
    }

    #[test]
    fn publication_snapshot_accepts_a_canonical_finalized_ancestor() {
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: 2,
            finalized_block_hash: [0x22; 32],
            index_revision: 7,
        };
        validate_publication_snapshot_anchor(&snapshot, 5, Some([0x22; 32]), 9)
            .expect("a canonical ancestor remains valid while publication evidence finalizes");
    }

    #[test]
    fn publication_snapshot_rejects_future_or_noncanonical_anchors() {
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: 3,
            finalized_block_hash: [0x33; 32],
            index_revision: 4,
        };
        assert!(validate_publication_snapshot_anchor(&snapshot, 2, Some([0x33; 32]), 4).is_err());
        assert!(validate_publication_snapshot_anchor(&snapshot, 3, Some([0x44; 32]), 4).is_err());
        assert!(validate_publication_snapshot_anchor(&snapshot, 3, Some([0x33; 32]), 3).is_err());
    }

    #[test]
    fn archive_registration_replay_requires_the_exact_original_receipt() {
        let mut world = World::new();
        let publisher_key =
            KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).expect("publisher key");
        let publisher = AccountId::new(publisher_key.public_key().clone());
        let broker_key =
            KeyPair::try_from_seed(vec![0x32; 32], Algorithm::Ed25519).expect("broker key");
        let broker = AccountId::new(broker_key.public_key().clone());
        let provider = iroha_data_model::sorafs::capacity::ProviderId::new([0x33; 32]);
        world.provider_owners.insert(provider, broker.clone());
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let genesis = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("genesis height"),
            None,
            None,
            None,
            500,
            0,
        );
        let genesis_hash = genesis.hash();
        {
            let mut block_hashes = state.block_hashes.block();
            block_hashes.push_for_tests(genesis_hash);
            block_hashes.commit_for_tests();
        }
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(2).expect("replay height"),
            Some(genesis_hash),
            None,
            None,
            1_500,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let commitment = retention_archive(0x34).commitment;
        let binding = MusubiSeedIngressReceiptBindingV1 {
            chain_id: transaction.chain_id().clone(),
            genesis_block_hash: *genesis_hash.as_ref(),
            publisher: publisher.clone(),
            ingress_broker: broker,
            seed_provider: provider,
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([0x35; 32]),
            archive_id: commitment.archive_id(),
            car_body_digest: commitment.car_digest,
            car_body_length: commitment.car_size,
            nonce: [0x36; 32],
        };
        let signed_receipt =
            |binding: MusubiSeedIngressReceiptBindingV1, issued_at_ms, expires_at_ms| {
                let payload = MusubiSeedIngressReceiptPayloadV1 {
                    version: MUSUBI_REGISTRY_VERSION_V1,
                    binding,
                    issued_at_ms,
                    expires_at_ms,
                };
                MusubiSeedIngressReceiptV1 {
                    approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                        public_key: broker_key.public_key().clone(),
                        signature: SignatureOf::try_from_hash(
                            broker_key.private_key(),
                            payload.signing_hash(),
                        )
                        .expect("receipt signature"),
                    }],
                    payload,
                }
            };
        let registered_receipt = signed_receipt(binding.clone(), 500, 1_000);
        let archive_id = commitment.archive_id();
        transaction.world.musubi_archives.insert(
            archive_id,
            MusubiArchiveRecordV1 {
                archive_id,
                commitment: commitment.clone(),
                staging_receipt: registered_receipt.clone(),
                registered_by: publisher.clone(),
                registered_at_height: 1,
                location_revision: 1,
                location_ids: Vec::new(),
            },
        );

        RegisterMusubiArchiveV1::new(commitment.clone(), registered_receipt.clone(), 1)
            .execute(&publisher, &mut transaction)
            .expect("the exact registered receipt remains idempotent after expiry");
        assert_eq!(
            transaction
                .world
                .musubi_archives
                .get(&archive_id)
                .expect("registered archive")
                .staging_receipt,
            registered_receipt,
            "the first authoritative receipt remains immutable"
        );

        let refreshed_receipt = signed_receipt(binding.clone(), 1_400, 2_000);
        let error = RegisterMusubiArchiveV1::new(commitment.clone(), refreshed_receipt, 1)
            .execute(&publisher, &mut transaction)
            .expect_err("a refreshed receipt must not replace the registered receipt");
        assert!(
            error
                .to_string()
                .contains("different commitment or staging receipt")
        );

        let mut different_binding = binding;
        different_binding.nonce = [0xee; 32];
        let different_receipt = signed_receipt(different_binding, 1_400, 2_000);
        let error = RegisterMusubiArchiveV1::new(commitment, different_receipt, 1)
            .execute(&publisher, &mut transaction)
            .expect_err("a different operation nonce must not cross archive registration");
        assert!(
            error
                .to_string()
                .contains("different commitment or staging receipt")
        );
    }

    fn package(name: &str) -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            iroha_data_model::nexus::DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            name.parse().expect("package name"),
        )
    }

    fn seed_package_owner(
        package: &MusubiPackageIdV1,
        owner: &AccountId,
        governance_revision: u64,
        transaction: &mut StateTransaction<'_, '_>,
    ) {
        transaction.world.musubi_packages.insert(
            package.clone(),
            MusubiPackageRecordV1 {
                package: package.clone(),
                claimed_namespace: "sora".parse().expect("namespace"),
                claimed_namespace_binding: MusubiNamespaceBindingDigestV1::new([1; 32]),
                owners: vec![owner.clone()],
                member_accounts: vec![owner.clone()],
                claimed_at_height: 1,
                revisions: MusubiPackageRevisionsV1 {
                    governance: governance_revision,
                    metadata: 1,
                    archive_locations: 1,
                },
            },
        );
        let member = MusubiPackageMemberV1 {
            package: package.clone(),
            account: owner.clone(),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 1,
            governance_revision,
        };
        transaction
            .world
            .musubi_package_members
            .insert(member.key(), member.clone());
        upsert_maintainer_directory(
            MusubiMaintainerDirectoryEntryV1::Accepted(member),
            transaction,
        );
    }

    fn seed_pending_invitation(
        invitation: MusubiMaintainerInvitationV1,
        transaction: &mut StateTransaction<'_, '_>,
    ) {
        transaction
            .world
            .musubi_package_invitations
            .insert(invitation.invite_id, invitation.clone());
        upsert_maintainer_directory(
            MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation),
            transaction,
        );
    }

    fn take_musubi_events(transaction: &mut StateTransaction<'_, '_>) -> Vec<MusubiEvent> {
        transaction
            .world
            .take_external_events()
            .into_iter()
            .filter_map(|event| match event {
                iroha_data_model::events::EventBox::Data(data) => match data.as_ref() {
                    DataEvent::Musubi(event) => Some(event.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    fn decision_for_current_block(
        decision_id: [u8; 32],
        action: &MusubiParliamentActionV1,
        transaction: &StateTransaction<'_, '_>,
    ) -> MusubiGovernanceDecisionV1 {
        let execute_after_height = execution_height(transaction);
        let delay = transaction.gov.min_enactment_delay.max(1);
        let enacted_at_height = execute_after_height
            .checked_sub(delay)
            .filter(|height| *height > 0)
            .expect("fixture block leaves a positive enactment height");
        MusubiGovernanceDecisionV1 {
            decision_id,
            action_digest: action.action_digest(),
            enacted_at_height,
            execute_after_height,
        }
    }

    fn insert_enacted_proposal(
        decision_id: [u8; 32],
        kind: ProposalKind,
        enacted_at_height: u64,
        transaction: &mut StateTransaction<'_, '_>,
    ) {
        transaction.world.put_governance_proposal(
            decision_id,
            GovernanceProposalRecord {
                proposer: account(80),
                kind,
                created_height: enacted_at_height.saturating_sub(1).max(1),
                status: GovernanceProposalStatus::Enacted,
                pipeline: GovernancePipeline::default(),
                parliament_snapshot: None,
                finalization_evidence: None,
                enacted_at_height: Some(enacted_at_height),
            },
        );
    }

    fn seed_enacted_decision(
        action: &MusubiParliamentActionV1,
        transaction: &mut StateTransaction<'_, '_>,
    ) -> MusubiGovernanceDecisionV1 {
        let kind = ProposalKind::MusubiRegistryGovernance(action.clone());
        let decision_id = kind.fingerprint();
        let decision = decision_for_current_block(decision_id, action, transaction);
        insert_enacted_proposal(decision_id, kind, decision.enacted_at_height, transaction);
        decision
    }

    fn snapshot(revision: u64) -> MusubiRegistrySnapshotV1 {
        MusubiRegistrySnapshotV1 {
            finalized_height: 7,
            finalized_block_hash: [7; 32],
            index_revision: revision,
        }
    }

    fn retention_archive(seed: u8) -> MusubiArchiveRecordV1 {
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid: iroha_data_model::sorafs::pin_registry::ManifestRootCid::from_blake3_digest(
                [seed; 32],
            )
            .expect("retention fixture root CID"),
            chunker: iroha_data_model::sorafs::pin_registry::ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 0x1f,
            },
            chunk_plan_digest: MusubiContentDigestV1::new([seed.wrapping_add(1); 32]),
            por_root: MusubiContentDigestV1::new([seed.wrapping_add(2); 32]),
            content_length: 1,
            car_digest: MusubiContentDigestV1::new([seed.wrapping_add(3); 32]),
            car_size: 1,
            bundle_digest: MusubiContentDigestV1::new([seed.wrapping_add(4); 32]),
            source_tree_digest: MusubiContentDigestV1::new([seed.wrapping_add(5); 32]),
            descriptor_digest: MusubiContentDigestV1::new([seed.wrapping_add(6); 32]),
            file_count: 1,
            chunk_count: 1,
        };
        let archive_id = commitment.archive_id();
        let publisher = account(seed);
        let broker_keypair =
            KeyPair::try_from_seed(vec![seed.wrapping_add(1); 32], Algorithm::Ed25519)
                .expect("retention fixture broker keypair");
        let broker = AccountId::new(broker_keypair.public_key().clone());
        let receipt_payload = MusubiSeedIngressReceiptPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: MusubiSeedIngressReceiptBindingV1 {
                chain_id: iroha_data_model::ChainId::from("retention-test"),
                genesis_block_hash: [seed.wrapping_add(7); 32],
                publisher: publisher.clone(),
                ingress_broker: broker,
                seed_provider: iroha_data_model::sorafs::capacity::ProviderId::new(
                    [seed.wrapping_add(8); 32],
                ),
                semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new(
                    [seed.wrapping_add(9); 32],
                ),
                archive_id,
                car_body_digest: commitment.car_digest,
                car_body_length: commitment.car_size,
                nonce: [seed.wrapping_add(10); 32],
            },
            issued_at_ms: 1,
            expires_at_ms: 2,
        };
        let receipt_approval = MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker_keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(
                broker_keypair.private_key(),
                receipt_payload.signing_hash(),
            )
            .expect("sign retention fixture receipt"),
        };
        MusubiArchiveRecordV1 {
            archive_id,
            commitment: commitment.clone(),
            staging_receipt: MusubiSeedIngressReceiptV1 {
                payload: receipt_payload,
                approvals: vec![receipt_approval],
            },
            registered_by: publisher,
            registered_at_height: 1,
            location_revision: 1,
            location_ids: Vec::new(),
        }
    }

    fn retention_release(
        archive_id: ArchiveId,
        version: &str,
        yanked: bool,
        artifact_governance: MusubiArtifactGovernanceStateV1,
    ) -> MusubiReleaseRecordV1 {
        let release = MusubiReleaseIdV1::new(
            package("retention"),
            version.parse().expect("retention release version"),
        );
        let manifest = MusubiReleaseManifestV1 {
            release: release.clone(),
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([0xA1; 32]).expect("retention ABI"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([0xA2; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            archive_id,
            verification_lock_digest: MusubiVerificationLockDigestV1::new([0xA3; 32]),
        };
        MusubiReleaseRecordV1 {
            release_digest: manifest.release_digest(),
            manifest,
            published_by: account(111),
            published_at_height: 1,
            yank: MusubiReleaseYankV1 {
                release,
                yanked,
                reason: "retention fixture".parse().expect("yank reason"),
                changed_by: account(111),
                changed_at_height: 1,
                revision: 1,
            },
            artifact_governance,
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: 1,
            },
        }
    }

    fn exact_release_query_fixture(
        include_home: bool,
        include_universal: bool,
    ) -> (State, MusubiExactReleaseQueryV1) {
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero genesis height"),
            None,
            None,
            None,
            0,
            0,
        );
        let genesis_hash = header.hash();
        let archive = retention_archive(0x71);
        let release = retention_release(
            archive.archive_id,
            "1.2.3",
            false,
            MusubiArtifactGovernanceStateV1::Available,
        );
        let release_id = release.manifest.release.clone();
        let universal_release = MusubiResolverReleaseRowV1 {
            release: release_id.clone(),
            release_digest: release.release_digest,
            archive_id: archive.archive_id,
            source_digest: archive.commitment.source_tree_digest,
            interface_digest: release.manifest.interface_digest,
            abi: release.manifest.abi,
            dependencies: release.manifest.dependencies.clone(),
            selection: MusubiReleaseSelectionStateV1 {
                yank: release.yank.clone(),
                storage: MusubiArchiveAvailabilityV1 {
                    archive_id: archive.archive_id,
                    availability: MusubiStorageAvailabilityV1::Selectable,
                    healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                    active_locations: 1,
                    finalized_height: 1,
                    finalized_block_hash: *genesis_hash.as_ref(),
                    index_revision: 1,
                },
                governance: release.artifact_governance.clone(),
            },
            index_revision: 1,
        };
        let mut world = World::new();
        if include_home {
            world.musubi_releases.insert(release_id.clone(), release);
        }
        if include_universal {
            world
                .musubi_resolver_index
                .insert(release_id.clone(), universal_release);
        }
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        {
            let mut block_hashes = state.block_hashes.block();
            block_hashes.push_for_tests(genesis_hash);
            block_hashes.commit_for_tests();
        }
        (
            state,
            MusubiExactReleaseQueryV1 {
                release: release_id,
            },
        )
    }

    #[test]
    fn exact_release_query_returns_paired_projections_from_one_snapshot() {
        let (state, request) = exact_release_query_fixture(true, true);
        let response = ValidSingularQuery::execute(
            &FindMusubiExactReleaseV1::new(request.clone()),
            &state.view(),
        )
        .expect("paired exact release query");

        response
            .validate_for(&request)
            .expect("paired response validates for its request");
        assert_eq!(response.snapshot.finalized_height, 1);
        assert_eq!(response.snapshot.index_revision, 1);
        assert_eq!(response.chain_id, state.chain_id_ref().clone());
        assert_eq!(response.home_release.manifest.release, request.release);
        assert_eq!(response.universal_release.release, request.release);
    }

    #[test]
    fn exact_release_query_fails_closed_for_one_sided_projection() {
        for (include_home, include_universal) in [(true, false), (false, true)] {
            let (state, request) = exact_release_query_fixture(include_home, include_universal);
            let error =
                ValidSingularQuery::execute(&FindMusubiExactReleaseV1::new(request), &state.view())
                    .expect_err("one-sided exact release projection must fail closed");
            assert!(matches!(
                error,
                QueryExecutionFail::Conversion(message)
                    if message.contains("home and universal projections are inconsistent")
            ));
        }
    }

    #[test]
    fn exact_release_query_reports_not_found_only_when_both_projections_are_absent() {
        let (state, request) = exact_release_query_fixture(false, false);
        let error =
            ValidSingularQuery::execute(&FindMusubiExactReleaseV1::new(request), &state.view())
                .expect_err("absent exact release must be reported as not found");
        assert_eq!(error, QueryExecutionFail::NotFound);
    }

    fn seed_retention_archive(
        world: &mut World,
        archive: MusubiArchiveRecordV1,
        releases: Vec<MusubiReleaseRecordV1>,
    ) -> ArchiveId {
        let archive_id = archive.archive_id;
        let mut release_ids = releases
            .iter()
            .map(|release| release.manifest.release.clone())
            .collect::<Vec<_>>();
        release_ids.sort();
        let release_count =
            u64::try_from(release_ids.len()).expect("release fixture count fits u64");
        for release in releases {
            world
                .musubi_releases
                .insert(release.manifest.release.clone(), release);
        }
        world.musubi_archives.insert(archive_id, archive);
        world.musubi_archive_availability.insert(
            archive_id,
            MusubiArchiveAvailabilityV1 {
                archive_id,
                availability: MusubiStorageAvailabilityV1::Unavailable,
                healthy_replicas: 0,
                active_locations: 0,
                finalized_height: 1,
                finalized_block_hash: [0xB1; 32],
                index_revision: 1,
            },
        );
        world.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: release_ids,
            },
        );
        let shortfall = *world.musubi_replication_shortfall_releases.view().get();
        world.musubi_replication_shortfall_releases = Cell::new(
            shortfall
                .checked_add(release_count)
                .expect("retention fixture shortfall count fits u64"),
        );
        archive_id
    }

    fn archive_location_replay_fixture(
        seed: u8,
    ) -> (
        World,
        AccountId,
        MusubiArchiveLocationKeyV1,
        AddMusubiArchiveLocationV1,
    ) {
        let mut world = World::new();
        let mut archive = retention_archive(seed);
        let genesis_hash = archive_location_genesis_header().hash();
        archive.staging_receipt.payload.binding.genesis_block_hash = *genesis_hash.as_ref();
        let broker_keypair =
            KeyPair::try_from_seed(vec![seed.wrapping_add(1); 32], Algorithm::Ed25519)
                .expect("fixture ingress broker keypair");
        archive.staging_receipt.approvals[0].signature = SignatureOf::try_from_hash(
            broker_keypair.private_key(),
            archive.staging_receipt.payload.signing_hash(),
        )
        .expect("resign fixture ingress receipt");
        let authority = archive.registered_by.clone();
        let location_id = MusubiArchiveLocationIdV1::new([seed.wrapping_add(11); 32]);
        let pin = iroha_data_model::sorafs::pin_registry::ManifestDigest::new(
            [seed.wrapping_add(12); 32],
        );
        let order = iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new(
            [seed.wrapping_add(13); 32],
        );
        archive.location_revision = 7;
        archive.location_ids = vec![location_id];
        let archive_id = archive.archive_id;
        let key = MusubiArchiveLocationKeyV1::new(archive_id, location_id);
        let mut location = location_fixture(seed, pin, order);
        location.archive_id = archive_id;
        location.location_id = location_id;
        location.revision = archive.location_revision;
        location.state = MusubiArchiveLocationStateV1::Degraded;
        let provider_keypair =
            KeyPair::try_from_seed(vec![seed.wrapping_add(14); 32], Algorithm::Ed25519)
                .expect("fixture provider keypair");
        let provider_owner = AccountId::new(provider_keypair.public_key().clone());
        let provider_id =
            iroha_data_model::sorafs::capacity::ProviderId::new([seed.wrapping_add(15); 32]);
        let completion_authority =
            iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionAuthorityV1::new(
                provider_owner.clone(),
                iroha_data_model::sorafs::pin_registry::ProviderIngestCompletionSignerPolicyV1 {
                    policy_id: [seed.wrapping_add(16); 32],
                    revision: 1,
                    predecessor_digest: None,
                    policy_digest: [seed.wrapping_add(17); 32],
                },
            );
        let binding = MusubiProviderBundleVerificationBindingV1 {
            chain_id: archive.staging_receipt.payload.binding.chain_id.clone(),
            genesis_block_hash: archive.staging_receipt.payload.binding.genesis_block_hash,
            provider_id,
            completed_by: provider_owner.clone(),
            completion_authority,
            replication_order: order,
            assignment_revision: 1,
            completion_epoch: 1,
            finalized_anchor:
                iroha_data_model::sorafs::pin_registry::ProviderIngestFinalizedAnchorV1 {
                    height: 1,
                    block_hash: *genesis_hash.as_ref(),
                },
            archive_id,
            bundle_digest: archive.commitment.bundle_digest,
            descriptor_digest: archive.commitment.descriptor_digest,
            semantic_release_manifest_digest: archive
                .staging_receipt
                .payload
                .binding
                .semantic_release_manifest_digest,
            verification_lock_digest: MusubiVerificationLockDigestV1::new(
                [seed.wrapping_add(19); 32],
            ),
            source_tree_digest: archive.commitment.source_tree_digest,
        };
        let payload = MusubiProviderBundleVerificationPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: binding.clone(),
        };
        let attestation = MusubiProviderBundleVerificationAttestationV1 {
            approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
                public_key: provider_keypair.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    provider_keypair.private_key(),
                    payload.signing_hash(),
                )
                .expect("sign fixture provider attestation"),
            }],
            payload,
        };
        attestation
            .verify(&binding)
            .expect("fixture provider attestation is cryptographically valid");
        let attestation_key = attestation.key();
        let attestation_digest = attestation.digest();
        let provider_attestation_set_digest = musubi_provider_bundle_attestation_set_digest_v1(
            archive_id,
            order,
            &[attestation.reference()],
        )
        .expect("fixture provider attestation set digest");
        location.providers = vec![provider_id];
        location.provider_attestation_set_digest = provider_attestation_set_digest;
        location.finalized_height = 2;
        location
            .validate()
            .expect("fixture archive location is structurally valid");
        let instruction = AddMusubiArchiveLocationV1 {
            archive_id,
            location_id,
            pin_manifest: location.pin_manifest,
            replication_order: location.replication_order,
            provider_attestation_set_digest,
            renew_after_epoch: location.renew_after_epoch,
            expires_at_epoch: location.expires_at_epoch,
            expected_location_revision: 1,
        };
        let mut pin_record = iroha_data_model::sorafs::pin_registry::PinManifestRecord::new(
            pin,
            archive.commitment.root_cid.clone(),
            archive.commitment.chunker.clone(),
            *archive.commitment.chunk_plan_digest.as_bytes(),
            *archive.commitment.por_root.as_bytes(),
            archive.commitment.content_length,
            iroha_data_model::sorafs::pin_registry::PinPolicy {
                min_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
                retention_epoch: location.expires_at_epoch,
            },
            authority.clone(),
            1,
            None,
            None,
            iroha_data_model::metadata::Metadata::default(),
        );
        pin_record.approve(1, None);
        world.pin_manifests.insert(pin, pin_record);
        world.replication_orders.insert(
            order,
            iroha_data_model::sorafs::pin_registry::ReplicationOrderRecord {
                order_id: order,
                manifest_digest: pin,
                manifest_root_cid: archive.commitment.root_cid.clone(),
                issued_by: authority.clone(),
                issued_epoch: 1,
                deadline_epoch: location.expires_at_epoch,
                canonical_order: vec![seed],
                assignment_revision: binding.assignment_revision,
                provider_completions: vec![
                    iroha_data_model::sorafs::pin_registry::ReplicationOrderCompletionRecord {
                        provider_id,
                        completed_by: provider_owner.clone(),
                        completion_epoch: binding.completion_epoch,
                        assignment_revision: binding.assignment_revision,
                        completion_authority: binding.completion_authority.clone(),
                        finalized_anchor: binding.finalized_anchor,
                    },
                ],
                status: ReplicationOrderStatus::Completed(binding.completion_epoch),
            },
        );
        world.provider_owners.insert(provider_id, provider_owner);
        world.musubi_provider_bundle_attestations.insert(
            attestation_key,
            MusubiProviderBundleAttestationRecordV1 {
                key: attestation_key,
                attestation_digest,
                attestation,
                registered_by: authority.clone(),
                registered_at_height: 1,
            },
        );
        world.musubi_locations_by_pin.insert(
            pin,
            MusubiPinLocationReferenceV1 {
                pin_manifest: pin,
                location: key,
                active: true,
            },
        );
        world.musubi_locations_by_replication_order.insert(
            order,
            MusubiReplicationOrderLocationReferenceV1 {
                replication_order: order,
                location: key,
                active: true,
            },
        );
        world
            .musubi_locations_by_provider
            .insert(MusubiProviderLocationKeyV1::new(provider_id, key), ());
        world.musubi_archives.insert(archive_id, archive);
        world.musubi_archive_locations.insert(key, location);
        (world, authority, key, instruction)
    }

    #[test]
    fn archive_package_governance_replaces_the_prepublication_registrant_capability() {
        let former_registrant = account(0x51);
        let current_owner = account(0x52);
        let mut archive = retention_archive(0x53);
        archive.registered_by = former_registrant.clone();
        let governed_package = package("archive-recovery");
        let governed_release =
            MusubiReleaseIdV1::new(governed_package.clone(), "1.0.0".parse().expect("version"));
        let mut world = World::new();
        world.musubi_packages.insert(
            governed_package.clone(),
            MusubiPackageRecordV1 {
                package: governed_package.clone(),
                claimed_namespace: "sora".parse().expect("namespace"),
                claimed_namespace_binding: MusubiNamespaceBindingDigestV1::new([0x54; 32]),
                owners: vec![current_owner.clone()],
                member_accounts: vec![current_owner.clone()],
                claimed_at_height: 1,
                revisions: MusubiPackageRevisionsV1 {
                    governance: 2,
                    metadata: 1,
                    archive_locations: 2,
                },
            },
        );
        let owner = MusubiPackageMemberV1 {
            package: governed_package,
            account: current_owner.clone(),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 2,
            governance_revision: 2,
        };
        world.musubi_package_members.insert(owner.key(), owner);

        let mut reason = MusubiGovernanceRejectionReasonV1::Other;
        {
            let world_view = world.view();
            ensure_archive_manager(&archive, &former_registrant, &world_view, &mut reason)
                .expect("the archive registrant manages an unpublished archive");
        }

        world.musubi_archive_reverse_references.insert(
            archive.archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id: archive.archive_id,
                releases: vec![governed_release],
            },
        );
        let world_view = world.view();
        ensure_archive_manager(&archive, &current_owner, &world_view, &mut reason)
            .expect("the current package owner manages a published archive");
        let error = ensure_archive_manager(&archive, &former_registrant, &world_view, &mut reason)
            .expect_err("a removed registrant must not retain archive-location authority");
        assert!(error.to_string().contains("lacks the required"));
        assert_eq!(reason, MusubiGovernanceRejectionReasonV1::Unauthorized);
    }

    #[test]
    fn explicit_location_invalidation_preserves_a_protected_archives_replica_quorum() {
        let (mut world, _, remaining_key, _) = archive_location_replay_fixture(0x55);
        let archive_id = remaining_key.archive_id;
        let remaining = world
            .musubi_archive_locations
            .view()
            .get(&remaining_key)
            .cloned()
            .expect("fixture remaining location");
        assert_eq!(
            current_location_providers(&remaining, &world.view())
                .expect("fixture evidence is current")
                .len(),
            1
        );
        assert!(!provider_count_is_healthy(2));
        assert!(provider_count_is_healthy(3));

        let invalidated_id = MusubiArchiveLocationIdV1::new([0x56; 32]);
        let invalidated_key = MusubiArchiveLocationKeyV1::new(archive_id, invalidated_id);
        let mut archive = world
            .musubi_archives
            .view()
            .get(&archive_id)
            .cloned()
            .expect("fixture archive");
        archive.location_ids.push(invalidated_id);
        archive.location_ids.sort();
        world.musubi_archives.insert(archive_id, archive);

        let release = retention_release(
            archive_id,
            "1.0.0",
            false,
            MusubiArtifactGovernanceStateV1::Available,
        );
        let release_id = release.manifest.release.clone();
        world.musubi_releases.insert(release_id.clone(), release);
        world.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: vec![release_id],
            },
        );

        let world_view = world.view();
        let error = ensure_locations_may_be_invalidated(&[invalidated_key], &world_view)
            .expect_err("one remaining fetchable replica is not a healthy release floor");
        assert!(error.to_string().contains("quorum-healthy"));
    }

    fn archive_location_genesis_header() -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero genesis height"),
            None,
            None,
            None,
            0,
            0,
        )
    }

    fn archive_location_replay_state(world: World) -> State {
        let state = State::new_with_chain_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            iroha_data_model::ChainId::from("retention-test"),
        );
        {
            let mut block_hashes = state.block_hashes.block();
            block_hashes.push_for_tests(archive_location_genesis_header().hash());
            block_hashes.commit_for_tests();
        }
        state
    }

    fn archive_location_replay_block(state: &State) -> crate::state::StateBlock<'_> {
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(2).expect("nonzero replay height"),
            None,
            None,
            None,
            0,
            0,
        );
        state.block(header)
    }

    #[test]
    fn provider_attestation_audit_query_loads_one_exact_immutable_record() {
        let (world, _, location_key, instruction) = archive_location_replay_fixture(0x40);
        let locations = world.musubi_archive_locations.view();
        let location = locations.get(&location_key).expect("fixture location");
        let key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: instruction.archive_id,
            replication_order: instruction.replication_order,
            provider_id: location.providers[0],
        };
        let expected = world
            .musubi_provider_bundle_attestations
            .view()
            .get(&key)
            .cloned()
            .expect("fixture provider attestation");
        let state = archive_location_replay_state(world);

        let actual = ValidSingularQuery::execute(
            &FindMusubiProviderBundleAttestationV1::new(key),
            &state.view(),
        )
        .expect("exact provider attestation audit query");
        assert_eq!(actual, expected);
    }

    #[test]
    fn provider_attestation_registration_is_exactly_idempotent_after_cas_consumption() {
        let (world, authority, location_key, instruction) = archive_location_replay_fixture(0x3A);
        let provider = world
            .musubi_archive_locations
            .view()
            .get(&location_key)
            .expect("fixture location")
            .providers[0];
        let key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: instruction.archive_id,
            replication_order: instruction.replication_order,
            provider_id: provider,
        };
        let record = world
            .musubi_provider_bundle_attestations
            .view()
            .get(&key)
            .cloned()
            .expect("fixture provider attestation");
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();

        RegisterMusubiProviderBundleAttestationV1::new(record.attestation.clone(), 1)
            .execute(&authority, &mut transaction)
            .expect("exact retry ignores the consumed location CAS revision");
        assert_eq!(
            transaction
                .world
                .musubi_provider_bundle_attestations
                .get(&key),
            Some(&record)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn successor_archive_manager_can_replay_identical_provider_evidence_without_rewriting_audit() {
        let (world, former_manager, location_key, instruction) =
            archive_location_replay_fixture(0x3E);
        let provider = world
            .musubi_archive_locations
            .view()
            .get(&location_key)
            .expect("fixture location")
            .providers[0];
        let key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: instruction.archive_id,
            replication_order: instruction.replication_order,
            provider_id: provider,
        };
        let record = world
            .musubi_provider_bundle_attestations
            .view()
            .get(&key)
            .cloned()
            .expect("fixture provider attestation");
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();
        let successor = account(0x3F);
        let governed_package = package("attestation-recovery");
        seed_package_owner(&governed_package, &successor, 2, &mut transaction);
        transaction.world.musubi_archive_reverse_references.insert(
            instruction.archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id: instruction.archive_id,
                releases: vec![MusubiReleaseIdV1::new(
                    governed_package,
                    "1.0.0".parse().expect("version"),
                )],
            },
        );
        let replay = RegisterMusubiProviderBundleAttestationV1::new(record.attestation.clone(), 1);

        replay
            .clone()
            .execute(&successor, &mut transaction)
            .expect("a current successor manager may replay identical immutable evidence");
        assert_eq!(
            transaction
                .world
                .musubi_provider_bundle_attestations
                .get(&key),
            Some(&record),
            "an idempotent successor replay must retain the original registrant audit"
        );
        assert!(take_musubi_events(&mut transaction).is_empty());

        let error = replay
            .execute(&former_manager, &mut transaction)
            .expect_err("the removed former manager must not retain replay authority");
        assert!(error.to_string().contains("lacks the required"));
        assert_eq!(
            transaction
                .world
                .musubi_provider_bundle_attestations
                .get(&key),
            Some(&record)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn new_provider_attestation_registration_requires_the_current_location_revision() {
        let (world, authority, location_key, instruction) = archive_location_replay_fixture(0x3B);
        let provider = world
            .musubi_archive_locations
            .view()
            .get(&location_key)
            .expect("fixture location")
            .providers[0];
        let key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: instruction.archive_id,
            replication_order: instruction.replication_order,
            provider_id: provider,
        };
        let record = world
            .musubi_provider_bundle_attestations
            .view()
            .get(&key)
            .cloned()
            .expect("fixture provider attestation");
        let mut attestations = world.musubi_provider_bundle_attestations.block();
        let _ = attestations.remove(key);
        attestations.commit();
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();

        let error = RegisterMusubiProviderBundleAttestationV1::new(record.attestation, 1)
            .execute(&authority, &mut transaction)
            .expect_err("new immutable evidence must use the current location revision");
        assert!(
            error
                .to_string()
                .contains("stale Musubi archive location revision")
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn archive_location_add_recomputes_the_registered_attestation_set_digest() {
        let (world, authority, _, mut instruction) = archive_location_replay_fixture(0x3C);
        instruction.expected_location_revision = 7;
        instruction.provider_attestation_set_digest =
            MusubiProviderBundleAttestationSetDigestV1::new([0xEE; 32]);
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();

        let error = instruction
            .execute(&authority, &mut transaction)
            .expect_err("a substituted compact attestation-set digest must be rejected");
        assert!(
            error
                .to_string()
                .contains("does not cover the finalized completion set")
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn archive_location_add_requires_attestation_records_from_an_earlier_finalized_height() {
        let (mut world, authority, key, mut instruction) = archive_location_replay_fixture(0x3D);
        let provider = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .expect("fixture location")
            .providers[0];
        let attestation_key = MusubiProviderBundleAttestationKeyV1 {
            archive_id: key.archive_id,
            replication_order: instruction.replication_order,
            provider_id: provider,
        };
        let record = world
            .musubi_provider_bundle_attestations
            .view()
            .get(&attestation_key)
            .cloned()
            .expect("fixture provider attestation");
        let mut attestations = world.musubi_provider_bundle_attestations.block();
        let _ = attestations.remove(attestation_key);
        attestations.commit();
        let mut archive = world
            .musubi_archives
            .view()
            .get(&key.archive_id)
            .cloned()
            .expect("fixture archive");
        archive.location_ids.clear();
        world.musubi_archives.insert(key.archive_id, archive);
        let mut locations = world.musubi_archive_locations.block();
        let _ = locations.remove(key);
        locations.commit();
        instruction.expected_location_revision = 7;
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();

        RegisterMusubiProviderBundleAttestationV1::new(record.attestation, 7)
            .execute(&authority, &mut transaction)
            .expect("current-revision provider evidence registers immutably");

        let error = instruction
            .execute(&authority, &mut transaction)
            .expect_err("same-height provider evidence is not finalized for location admission");
        assert!(
            error
                .to_string()
                .contains("must be finalized before archive location admission")
        );
        assert!(matches!(
            take_musubi_events(&mut transaction).as_slice(),
            [MusubiEvent::ProviderBundleAttestationRegistered(_)]
        ));
    }

    fn assert_exact_archive_location_replay_rejects_corruption(
        seed: u8,
        corrupt: impl FnOnce(&mut World, MusubiArchiveLocationKeyV1, &mut AddMusubiArchiveLocationV1),
        expected_message: &str,
    ) {
        let (mut world, authority, key, mut instruction) = archive_location_replay_fixture(seed);
        corrupt(&mut world, key, &mut instruction);
        let archive_id = instruction.archive_id;
        let archive_before = world
            .musubi_archives
            .view()
            .get(&archive_id)
            .cloned()
            .expect("fixture archive");
        let location_before = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .cloned()
            .expect("fixture location");
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();

        let error = instruction
            .execute(&authority, &mut transaction)
            .expect_err("corrupt authoritative replay state must fail closed");

        assert!(
            error.to_string().contains(expected_message),
            "unexpected replay error: {error}"
        );
        assert_eq!(
            transaction.world.musubi_archives.get(&archive_id),
            Some(&archive_before)
        );
        assert_eq!(
            transaction.world.musubi_archive_locations.get(&key),
            Some(&location_before)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn exact_archive_location_replay_ignores_stale_revision_without_mutation() {
        let seed = 0x41;
        let (world, authority, key, instruction) = archive_location_replay_fixture(seed);
        let archive_id = instruction.archive_id;
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();
        let archive_before = transaction
            .world
            .musubi_archives
            .get(&archive_id)
            .cloned()
            .expect("fixture archive");
        let location_before = transaction
            .world
            .musubi_archive_locations
            .get(&key)
            .cloned()
            .expect("fixture location");

        instruction
            .execute(&authority, &mut transaction)
            .expect("an exact replay must not require the consumed CAS revision");

        assert_eq!(
            transaction.world.musubi_archives.get(&archive_id),
            Some(&archive_before)
        );
        assert_eq!(
            transaction.world.musubi_archive_locations.get(&key),
            Some(&location_before)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn exact_archive_location_replay_rejects_a_malformed_stored_location() {
        let seed = 0x44;
        let (mut world, authority, key, instruction) = archive_location_replay_fixture(seed);
        let mut malformed = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .cloned()
            .expect("fixture location");
        malformed.providers.clear();
        world
            .musubi_archive_locations
            .insert(key, malformed.clone());
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();

        let error = instruction
            .execute(&authority, &mut transaction)
            .expect_err("an exact replay must not bless malformed authoritative state");

        assert!(error.to_string().contains("archive location is invalid"));
        assert_eq!(
            transaction.world.musubi_archive_locations.get(&key),
            Some(&malformed)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn exact_archive_location_replay_rejects_a_future_location_revision() {
        assert_exact_archive_location_replay_rejects_corruption(
            0x4A,
            |world, key, _| {
                let mut location = world
                    .musubi_archive_locations
                    .view()
                    .get(&key)
                    .cloned()
                    .expect("fixture location");
                location.revision = world
                    .musubi_archives
                    .view()
                    .get(&key.archive_id)
                    .expect("fixture archive")
                    .location_revision
                    .checked_add(1)
                    .expect("fixture revision remains bounded");
                world.musubi_archive_locations.insert(key, location);
            },
            "location revision or finalized height is inconsistent",
        );
    }

    #[test]
    fn exact_archive_location_replay_rejects_mismatched_immutable_attestation_evidence() {
        assert_exact_archive_location_replay_rejects_corruption(
            0x4B,
            |world, key, _| {
                let location = world
                    .musubi_archive_locations
                    .view()
                    .get(&key)
                    .cloned()
                    .expect("fixture location");
                let attestation_key = MusubiProviderBundleAttestationKeyV1 {
                    archive_id: key.archive_id,
                    replication_order: location.replication_order,
                    provider_id: location.providers[0],
                };
                let mut record = world
                    .musubi_provider_bundle_attestations
                    .view()
                    .get(&attestation_key)
                    .cloned()
                    .expect("fixture provider attestation");
                record.attestation.payload.binding.bundle_digest =
                    MusubiContentDigestV1::new([0xEE; 32]);
                record.attestation_digest = record.attestation.digest();
                world
                    .musubi_provider_bundle_attestations
                    .insert(attestation_key, record);
            },
            "attestation does not match its immutable archive commitments",
        );
    }

    #[test]
    fn exact_archive_location_replay_rejects_an_invalid_stored_attestation_signature() {
        assert_exact_archive_location_replay_rejects_corruption(
            0x4C,
            |world, key, _| {
                let location = world
                    .musubi_archive_locations
                    .view()
                    .get(&key)
                    .cloned()
                    .expect("fixture location");
                let attestation_key = MusubiProviderBundleAttestationKeyV1 {
                    archive_id: key.archive_id,
                    replication_order: location.replication_order,
                    provider_id: location.providers[0],
                };
                let mut record = world
                    .musubi_provider_bundle_attestations
                    .view()
                    .get(&attestation_key)
                    .cloned()
                    .expect("fixture provider attestation");
                let foreign_keypair = KeyPair::try_from_seed(vec![0xFD; 32], Algorithm::Ed25519)
                    .expect("foreign fixture keypair");
                record.attestation.approvals[0].public_key = foreign_keypair.public_key().clone();
                record.attestation_digest = record.attestation.digest();
                world
                    .musubi_provider_bundle_attestations
                    .insert(attestation_key, record);
            },
            "approval is not a provider-owner key",
        );
    }

    #[test]
    fn exact_archive_location_replay_rejects_a_missing_pin_reverse_reference() {
        assert_exact_archive_location_replay_rejects_corruption(
            0x45,
            |world, _, instruction| {
                let mut block = world.musubi_locations_by_pin.block();
                let _ = block.remove(instruction.pin_manifest);
                block.commit();
            },
            "pin reverse index is inconsistent",
        );
    }

    #[test]
    fn exact_archive_location_replay_rejects_an_inactive_order_reverse_reference() {
        assert_exact_archive_location_replay_rejects_corruption(
            0x46,
            |world, _, instruction| {
                let mut reference = world
                    .musubi_locations_by_replication_order
                    .view()
                    .get(&instruction.replication_order)
                    .copied()
                    .expect("fixture order reverse reference");
                reference.active = false;
                world
                    .musubi_locations_by_replication_order
                    .insert(instruction.replication_order, reference);
            },
            "order reverse index is inconsistent",
        );
    }

    #[test]
    fn exact_archive_location_replay_rejects_a_missing_provider_reverse_reference() {
        assert_exact_archive_location_replay_rejects_corruption(
            0x47,
            |world, key, _| {
                let provider = world
                    .musubi_archive_locations
                    .view()
                    .get(&key)
                    .expect("fixture location")
                    .providers[0];
                let mut block = world.musubi_locations_by_provider.block();
                let _ = block.remove(MusubiProviderLocationKeyV1::new(provider, key));
                block.commit();
            },
            "provider reverse index is inconsistent",
        );
    }

    #[test]
    fn exact_archive_location_replay_ignores_mutable_sorafs_degradation() {
        let seed = 0x48;
        let (mut world, authority, key, instruction) = archive_location_replay_fixture(seed);
        let archive_id = instruction.archive_id;
        let mut pin = world
            .pin_manifests
            .view()
            .get(&instruction.pin_manifest)
            .cloned()
            .expect("fixture pin manifest");
        pin.retire(2, Some("fixture lifecycle degradation".to_owned()));
        world.pin_manifests.insert(instruction.pin_manifest, pin);
        let mut order = world
            .replication_orders
            .view()
            .get(&instruction.replication_order)
            .cloned()
            .expect("fixture replication order");
        order.status = ReplicationOrderStatus::Expired(2);
        world
            .replication_orders
            .insert(instruction.replication_order, order);
        let provider = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .expect("fixture location")
            .providers[0];
        let mut provider_owners = world.provider_owners.block();
        let _ = provider_owners.remove(provider);
        provider_owners.commit();
        let archive_before = world
            .musubi_archives
            .view()
            .get(&archive_id)
            .cloned()
            .expect("fixture archive");
        let location_before = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .cloned()
            .expect("fixture location");
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();

        instruction
            .execute(&authority, &mut transaction)
            .expect("later mutable SoraFS degradation must not invalidate an exact replay");

        assert_eq!(
            transaction.world.musubi_archives.get(&archive_id),
            Some(&archive_before)
        );
        assert_eq!(
            transaction.world.musubi_archive_locations.get(&key),
            Some(&location_before)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn changed_archive_location_replay_still_requires_current_revision() {
        let seed = 0x42;
        let (world, authority, key, mut instruction) = archive_location_replay_fixture(seed);
        let archive_id = instruction.archive_id;
        instruction.expires_at_epoch = instruction
            .expires_at_epoch
            .checked_add(1)
            .expect("fixture expiry remains bounded");
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();
        let archive_before = transaction
            .world
            .musubi_archives
            .get(&archive_id)
            .cloned()
            .expect("fixture archive");
        let location_before = transaction
            .world
            .musubi_archive_locations
            .get(&key)
            .cloned()
            .expect("fixture location");

        let error = instruction
            .execute(&authority, &mut transaction)
            .expect_err("changed location content must not bypass compare-and-set");

        assert!(
            error
                .to_string()
                .contains("stale Musubi archive location revision")
        );
        assert_eq!(
            transaction.world.musubi_archives.get(&archive_id),
            Some(&archive_before)
        );
        assert_eq!(
            transaction.world.musubi_archive_locations.get(&key),
            Some(&location_before)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn retired_archive_location_identity_rejects_exact_stale_replay() {
        let seed = 0x43;
        let (mut world, authority, key, instruction) = archive_location_replay_fixture(seed);
        let mut retired = world
            .musubi_archive_locations
            .view()
            .get(&key)
            .cloned()
            .expect("fixture location");
        retired.state = MusubiArchiveLocationStateV1::Retired;
        world.musubi_archive_locations.insert(key, retired.clone());
        let state = archive_location_replay_state(world);
        let mut block = archive_location_replay_block(&state);
        let mut transaction = block.transaction();

        let error = instruction
            .execute(&authority, &mut transaction)
            .expect_err("a retired location identity must never be replayed or reused");

        assert!(
            error
                .to_string()
                .contains("retired archive location identities")
        );
        assert_eq!(
            transaction.world.musubi_archive_locations.get(&key),
            Some(&retired)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn replication_shortfall_transition_is_checked_and_boundary_scoped() {
        use MusubiStorageAvailabilityV1::{BelowQuorum, Selectable, Unavailable};

        assert_eq!(
            plan_replication_shortfall_transition(5, Selectable, BelowQuorum, 3)
                .expect("selectable-to-shortfall transition"),
            Some(8)
        );
        assert_eq!(
            plan_replication_shortfall_transition(5, Selectable, Unavailable, 3)
                .expect("selectable-to-unavailable transition"),
            Some(8)
        );
        assert_eq!(
            plan_replication_shortfall_transition(5, BelowQuorum, Selectable, 3)
                .expect("shortfall-to-selectable transition"),
            Some(2)
        );
        assert_eq!(
            plan_replication_shortfall_transition(5, Unavailable, Selectable, 3)
                .expect("unavailable-to-selectable transition"),
            Some(2)
        );
        assert_eq!(
            plan_replication_shortfall_transition(5, BelowQuorum, Unavailable, 3)
                .expect("non-selectable transition"),
            None
        );
        assert_eq!(
            plan_replication_shortfall_transition(5, Selectable, BelowQuorum, 0)
                .expect("empty reverse-reference transition"),
            None
        );
        assert!(
            plan_replication_shortfall_transition(u64::MAX, Selectable, BelowQuorum, 1).is_err(),
            "consensus aggregate overflow must fail closed"
        );
        assert!(
            plan_replication_shortfall_transition(0, Unavailable, Selectable, 1).is_err(),
            "consensus aggregate underflow must fail closed"
        );
    }

    #[test]
    fn availability_refresh_preflights_resolver_rows_and_packages_before_mutation() {
        let mut world = World::new();
        let archive = retention_archive(17);
        let archive_id = archive.archive_id;
        let source_digest = archive.commitment.source_tree_digest;
        let release = retention_release(
            archive_id,
            "1.0.0",
            false,
            MusubiArtifactGovernanceStateV1::Available,
        );
        let release_id = release.manifest.release.clone();
        let resolver_release = release.clone();
        seed_retention_archive(&mut world, archive, vec![release]);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(2).expect("nonzero fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let availability_before = *transaction
            .world
            .musubi_archive_availability
            .get(&archive_id)
            .expect("fixture archive has availability");
        let index_revision_before = transaction.world.musubi_resolver_index_revision.get().get();
        let shortfall_before = *transaction
            .world
            .musubi_replication_shortfall_releases
            .get();

        let error = refresh_archive_availability(archive_id, &mut transaction)
            .expect_err("a reverse-referenced release must have an exact resolver row");

        assert!(error.to_string().contains("missing its exact resolver row"));
        assert!(
            transaction
                .world
                .musubi_resolver_index
                .get(&release_id)
                .is_none()
        );
        assert_eq!(
            transaction
                .world
                .musubi_archive_availability
                .get(&archive_id),
            Some(&availability_before)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            index_revision_before
        );
        assert_eq!(
            *transaction
                .world
                .musubi_replication_shortfall_releases
                .get(),
            shortfall_before
        );

        let row = MusubiResolverReleaseRowV1 {
            release: release_id.clone(),
            release_digest: resolver_release.release_digest,
            archive_id,
            source_digest,
            interface_digest: resolver_release.manifest.interface_digest,
            abi: resolver_release.manifest.abi,
            dependencies: resolver_release.manifest.dependencies.clone(),
            selection: MusubiReleaseSelectionStateV1 {
                yank: resolver_release.yank,
                storage: availability_before,
                governance: resolver_release.artifact_governance,
            },
            index_revision: index_revision_before,
        };
        row.validate().expect("fixture resolver row is canonical");
        transaction
            .world
            .musubi_resolver_index
            .insert(release_id, row);

        let error = refresh_archive_availability(archive_id, &mut transaction)
            .expect_err("a reverse-referenced release must retain its package record");
        assert!(error.to_string().contains("missing package record"));
        assert_eq!(
            transaction
                .world
                .musubi_archive_availability
                .get(&archive_id),
            Some(&availability_before)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            index_revision_before
        );
        assert_eq!(
            *transaction
                .world
                .musubi_replication_shortfall_releases
                .get(),
            shortfall_before
        );
    }

    #[test]
    fn availability_refresh_rejects_an_invalid_archive_before_mutation() {
        let mut world = World::new();
        let mut archive = retention_archive(18);
        let archive_id = archive.archive_id;
        archive.location_revision = 0;
        seed_retention_archive(&mut world, archive.clone(), Vec::new());
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(2).expect("nonzero fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let availability_before = *transaction
            .world
            .musubi_archive_availability
            .get(&archive_id)
            .expect("fixture archive has availability");
        let index_revision_before = transaction.world.musubi_resolver_index_revision.get().get();

        let error = refresh_archive_availability(archive_id, &mut transaction)
            .expect_err("an invalid authoritative archive must fail closed");

        assert!(error.to_string().contains("archive record"));
        assert_eq!(
            transaction.world.musubi_archives.get(&archive_id),
            Some(&archive)
        );
        assert_eq!(
            transaction
                .world
                .musubi_archive_availability
                .get(&archive_id),
            Some(&availability_before)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            index_revision_before
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn availability_refresh_rejects_a_mismatched_archive_identity_before_mutation() {
        let mut world = World::new();
        let canonical = retention_archive(19);
        let archive_id = canonical.archive_id;
        let mismatched = retention_archive(20);
        mismatched
            .validate()
            .expect("mismatched archive fixture is structurally valid");
        seed_retention_archive(&mut world, canonical, Vec::new());
        world.musubi_archives.insert(archive_id, mismatched.clone());
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(2).expect("nonzero fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let availability_before = *transaction
            .world
            .musubi_archive_availability
            .get(&archive_id)
            .expect("fixture archive has availability");
        let index_revision_before = transaction.world.musubi_resolver_index_revision.get().get();

        let error = refresh_archive_availability(archive_id, &mut transaction)
            .expect_err("an archive stored under another identity must fail closed");

        assert!(
            error
                .to_string()
                .contains("wrong embedded archive identity")
        );
        assert_eq!(
            transaction.world.musubi_archives.get(&archive_id),
            Some(&mismatched)
        );
        assert_eq!(
            transaction
                .world
                .musubi_archive_availability
                .get(&archive_id),
            Some(&availability_before)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            index_revision_before
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn availability_refresh_preflights_location_validation_and_identity() {
        let mut world = World::new();
        let mut archive = retention_archive(21);
        let archive_id = archive.archive_id;
        let location_id = MusubiArchiveLocationIdV1::new([0x51; 32]);
        archive.location_ids = vec![location_id];
        archive
            .validate()
            .expect("archive with one location identity is valid");
        seed_retention_archive(&mut world, archive.clone(), Vec::new());
        let key = MusubiArchiveLocationKeyV1::new(archive_id, location_id);
        let mut location = location_fixture(
            0x51,
            iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0x52; 32]),
            iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new([0x53; 32]),
        );
        location.archive_id = archive_id;
        location.location_id = location_id;
        world.musubi_archive_locations.insert(key, location.clone());
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(2).expect("nonzero fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let availability_before = *transaction
            .world
            .musubi_archive_availability
            .get(&archive_id)
            .expect("fixture archive has availability");
        let index_revision_before = transaction.world.musubi_resolver_index_revision.get().get();

        let invalid = refresh_archive_availability(archive_id, &mut transaction)
            .expect_err("a malformed location must fail before availability changes");
        assert!(invalid.to_string().contains("archive location is invalid"));
        assert_eq!(
            transaction.world.musubi_archive_locations.get(&key),
            Some(&location)
        );
        assert_eq!(
            transaction
                .world
                .musubi_archive_availability
                .get(&archive_id),
            Some(&availability_before)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            index_revision_before
        );
        assert!(take_musubi_events(&mut transaction).is_empty());

        location.location_id = MusubiArchiveLocationIdV1::new([0x54; 32]);
        transaction
            .world
            .musubi_archive_locations
            .insert(key, location.clone());
        let mismatched = refresh_archive_availability(archive_id, &mut transaction)
            .expect_err("a location stored under another identity must fail closed");
        assert!(mismatched.to_string().contains("wrong embedded identity"));
        assert_eq!(
            transaction.world.musubi_archive_locations.get(&key),
            Some(&location)
        );
        assert_eq!(
            transaction
                .world
                .musubi_archive_availability
                .get(&archive_id),
            Some(&availability_before)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            index_revision_before
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn archive_retention_finalized_time_requires_the_exact_snapshot_block() {
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: 7,
            finalized_block_hash: [0x71; 32],
            index_revision: 9,
        };
        assert_eq!(
            validated_finalized_block_time(&snapshot, 7, [0x71; 32], 1_700_000_000_000)
                .expect("exact finalized block"),
            1_700_000_000_000
        );
        assert!(validated_finalized_block_time(&snapshot, 8, [0x71; 32], 1).is_err());
        assert!(validated_finalized_block_time(&snapshot, 7, [0x72; 32], 1).is_err());
    }

    #[test]
    fn archive_retention_point_lookups_keep_active_yanked_and_unknown_archives() {
        let mut world = World::new();
        let unreferenced = seed_retention_archive(&mut world, retention_archive(11), Vec::new());
        let referenced_archive = retention_archive(21);
        let referenced = referenced_archive.archive_id;
        let takedown = |seed| {
            MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
                action_digest: MusubiGovernanceActionDigestV1::new([seed; 32]),
                reason: "Parliament fixture".parse().expect("takedown reason"),
                applied_at_height: 1,
            })
        };
        seed_retention_archive(
            &mut world,
            referenced_archive,
            vec![
                retention_release(
                    referenced,
                    "1.0.0",
                    false,
                    MusubiArtifactGovernanceStateV1::Available,
                ),
                retention_release(
                    referenced,
                    "1.1.0",
                    true,
                    MusubiArtifactGovernanceStateV1::Available,
                ),
                retention_release(referenced, "1.2.0", false, takedown(31)),
            ],
        );
        let taken_down_archive = retention_archive(41);
        let taken_down = taken_down_archive.archive_id;
        seed_retention_archive(
            &mut world,
            taken_down_archive,
            vec![retention_release(taken_down, "2.0.0", true, takedown(42))],
        );
        let view = world.view();

        let unknown = archive_retention_decision(ArchiveId::new([0xF1; 32]), &view)
            .expect("unknown decision");
        assert_eq!(
            unknown.disposition,
            MusubiArchiveRetentionDispositionV1::RetainUnknown
        );
        assert!(unknown.must_retain());

        let unreferenced =
            archive_retention_decision(unreferenced, &view).expect("unreferenced decision");
        assert_eq!(
            unreferenced.disposition,
            MusubiArchiveRetentionDispositionV1::PruneUnreferenced
        );

        let referenced =
            archive_retention_decision(referenced, &view).expect("referenced decision");
        assert_eq!(
            referenced.disposition,
            MusubiArchiveRetentionDispositionV1::RetainReferenced
        );
        assert_eq!(referenced.active_releases, 1);
        assert_eq!(referenced.yanked_releases, 1);
        assert_eq!(referenced.taken_down_releases, 1);
        assert!(referenced.must_retain());

        let taken_down =
            archive_retention_decision(taken_down, &view).expect("taken-down decision");
        assert_eq!(
            taken_down.disposition,
            MusubiArchiveRetentionDispositionV1::PruneGovernedTakedown
        );
        assert_eq!(taken_down.taken_down_releases, 1);
        assert!(!taken_down.must_retain());
    }

    #[test]
    fn archive_retention_point_lookups_reject_projection_identity_mismatches() {
        let mut world = World::new();
        let archive = retention_archive(51);
        let archive_id = seed_retention_archive(&mut world, archive.clone(), Vec::new());
        let other_archive_id = retention_archive(52).archive_id;
        let valid_storage = world
            .musubi_archive_availability
            .view()
            .get(&archive_id)
            .cloned()
            .expect("seeded archive availability");

        world
            .musubi_archives
            .insert(archive_id, retention_archive(52));
        assert!(archive_retention_decision(archive_id, &world.view()).is_err());
        world.musubi_archives.insert(archive_id, archive.clone());

        world.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id: other_archive_id,
                releases: Vec::new(),
            },
        );
        assert!(archive_retention_decision(archive_id, &world.view()).is_err());
        world.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: Vec::new(),
            },
        );

        let mut mismatched_storage = valid_storage.clone();
        mismatched_storage.archive_id = other_archive_id;
        world
            .musubi_archive_availability
            .insert(archive_id, mismatched_storage);
        assert!(archive_retention_decision(archive_id, &world.view()).is_err());
        world
            .musubi_archive_availability
            .insert(archive_id, valid_storage);

        let missing_release = retention_release(
            archive_id,
            "1.0.0",
            false,
            MusubiArtifactGovernanceStateV1::Available,
        )
        .manifest
        .release;
        world.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: vec![missing_release],
            },
        );
        assert!(archive_retention_decision(archive_id, &world.view()).is_err());

        let mut mismatched_release = retention_release(
            archive_id,
            "2.0.0",
            false,
            MusubiArtifactGovernanceStateV1::Available,
        );
        let referenced_release = mismatched_release.manifest.release.clone();
        mismatched_release.manifest.release = MusubiReleaseIdV1::new(
            package("retention"),
            "2.1.0".parse().expect("mismatched release version"),
        );
        mismatched_release.yank.release = mismatched_release.manifest.release.clone();
        mismatched_release.release_digest = mismatched_release.manifest.release_digest();
        world
            .musubi_releases
            .insert(referenced_release.clone(), mismatched_release);
        world.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: vec![referenced_release],
            },
        );
        assert!(archive_retention_decision(archive_id, &world.view()).is_err());

        let mut wrong_archive_release = retention_release(
            archive_id,
            "3.0.0",
            true,
            MusubiArtifactGovernanceStateV1::Available,
        );
        let referenced_release = wrong_archive_release.manifest.release.clone();
        wrong_archive_release.manifest.archive_id = other_archive_id;
        wrong_archive_release.release_digest = wrong_archive_release.manifest.release_digest();
        world
            .musubi_releases
            .insert(referenced_release.clone(), wrong_archive_release);
        world.musubi_archive_reverse_references.insert(
            archive_id,
            MusubiArchiveReverseReferencesV1 {
                archive_id,
                releases: vec![referenced_release],
            },
        );
        assert!(archive_retention_decision(archive_id, &world.view()).is_err());
    }

    #[test]
    fn pagination_continues_after_exact_last_key() {
        let request = MusubiPageRequestV1 {
            limit: 2,
            cursor: None,
        };
        let hash = query_hash(b"test", b"request");
        let rows = vec![
            ("a".to_owned(), 1_u8),
            ("b".to_owned(), 2_u8),
            ("c".to_owned(), 3_u8),
        ];
        let (first, cursor) =
            paginate(rows.clone(), &request, hash, snapshot(1)).expect("first page");
        assert_eq!(first, vec![1, 2]);
        let cursor = cursor.expect("continuation");
        assert_eq!(cursor.last_key, "b");

        let request = MusubiPageRequestV1 {
            limit: 2,
            cursor: Some(cursor),
        };
        let (second, cursor) = paginate(rows, &request, hash, snapshot(1)).expect("second page");
        assert_eq!(second, vec![3]);
        assert!(cursor.is_none());
    }

    #[test]
    fn pagination_preserves_exact_cursor_failure_reasons() {
        fn assert_reason<T>(
            result: Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), MusubiQueryExecutionErrorV1>,
            expected: MusubiCursorFailureV1,
        ) {
            let error = match result {
                Ok(_) => panic!("cursor must fail"),
                Err(error) => error,
            };
            assert_eq!(error.cursor_failure(), Some(expected));
            assert_eq!(error.into_query_error(), QueryExecutionFail::Expired);
        }

        let hash = query_hash(b"test", b"request");
        let cursor =
            |cursor_snapshot, query_hash, last_key: &str, caller| MusubiFinalizedCursorV1 {
                snapshot: cursor_snapshot,
                query_hash,
                last_key: last_key.to_owned(),
                caller,
            };
        let page = |cursor| MusubiPageRequestV1 {
            limit: 1,
            cursor: Some(cursor),
        };

        let mut changed_anchor = snapshot(1);
        changed_anchor.finalized_height += 1;
        assert_reason(
            paginate(
                vec![("a".to_owned(), 1_u8)],
                &page(cursor(snapshot(1), hash, "a", None)),
                hash,
                changed_anchor,
            ),
            MusubiCursorFailureV1::FinalizedAnchorMismatch,
        );
        assert_reason(
            paginate(
                vec![("a".to_owned(), 1_u8)],
                &page(cursor(snapshot(1), hash, "a", None)),
                hash,
                snapshot(2),
            ),
            MusubiCursorFailureV1::IndexRevisionMismatch,
        );
        assert_reason(
            paginate(
                vec![("a".to_owned(), 1_u8)],
                &page(cursor(
                    snapshot(1),
                    query_hash(b"other", b"request"),
                    "a",
                    None,
                )),
                hash,
                snapshot(1),
            ),
            MusubiCursorFailureV1::QueryMismatch,
        );
        let expected_caller = account(1);
        assert_reason(
            paginate_for_caller(
                vec![("a".to_owned(), 1_u8)],
                &page(cursor(snapshot(1), hash, "a", Some(account(2)))),
                hash,
                snapshot(1),
                Some(&expected_caller),
            ),
            MusubiCursorFailureV1::CallerMismatch,
        );
        assert_reason(
            paginate(
                vec![("a".to_owned(), 1_u8)],
                &page(cursor(snapshot(1), hash, "missing", None)),
                hash,
                snapshot(1),
            ),
            MusubiCursorFailureV1::LastKeyStale,
        );

        let invalid_version_cursor = page(cursor(snapshot(1), hash, "01.0.0", None));
        let error = package_release_page_start(&package("cursor-test"), &invalid_version_cursor)
            .expect_err("noncanonical version boundary must fail");
        assert_eq!(
            error.cursor_failure(),
            Some(MusubiCursorFailureV1::LastKeyStale)
        );
    }

    #[test]
    fn query_hash_is_domain_separated() {
        assert_ne!(
            query_hash(b"versions", b"same"),
            query_hash(b"maintainers", b"same")
        );
        assert_ne!(
            query_hash(b"versions", b"same"),
            query_hash(b"versions", b"different")
        );
    }

    #[test]
    fn concurrent_pending_invitations_rebase_and_accept_independently() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(21);
        let first_account = account(22);
        let second_account = account(23);
        let package = package("concurrent-invites");
        let first_id = MusubiInviteIdV1::new([0x21; 32]);
        let second_id = MusubiInviteIdV1::new([0x22; 32]);
        seed_package_owner(&package, &owner, 1, &mut transaction);

        InviteMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: first_id,
            invited_account: first_account.clone(),
            role: MusubiPackageRoleV1::Owner,
            expires_at_height: 100,
            expected_governance_revision: 1,
        }
        .execute(&owner, &mut transaction)
        .expect("first invitation advances package governance");
        InviteMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: second_id,
            invited_account: second_account.clone(),
            role: MusubiPackageRoleV1::Owner,
            expires_at_height: 100,
            expected_governance_revision: 2,
        }
        .execute(&owner, &mut transaction)
        .expect("second invitation rebases the first invitation");

        for invite_id in [first_id, second_id] {
            let invitation = transaction
                .world
                .musubi_package_invitations
                .get(&invite_id)
                .expect("pending invitation remains authoritative");
            assert_eq!(invitation.state, MusubiInvitationStateV1::Pending);
            assert_eq!(invitation.expected_governance_revision, 3);
        }

        AcceptMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: first_id,
            expected_governance_revision: 3,
        }
        .execute(&first_account, &mut transaction)
        .expect("the rebased first invitation remains acceptable");
        assert_eq!(
            transaction
                .world
                .musubi_package_invitations
                .get(&first_id)
                .expect("accepted invitation remains in history")
                .state,
            MusubiInvitationStateV1::Accepted
        );
        let second = transaction
            .world
            .musubi_package_invitations
            .get(&second_id)
            .expect("second invitation remains pending");
        assert_eq!(second.state, MusubiInvitationStateV1::Pending);
        assert_eq!(second.expected_governance_revision, 4);

        AcceptMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: second_id,
            expected_governance_revision: 4,
        }
        .execute(&second_account, &mut transaction)
        .expect("the second invitation remains independently acceptable");
        let package = transaction
            .world
            .musubi_packages
            .get(&package)
            .expect("package remains after both acceptances");
        assert_eq!(package.revisions.governance, 5);
        assert!(package.owners.binary_search(&first_account).is_ok());
        assert!(package.owners.binary_search(&second_account).is_ok());
        assert_eq!(
            transaction
                .world
                .musubi_package_invitations
                .get(&second_id)
                .expect("second invitation remains in history")
                .state,
            MusubiInvitationStateV1::Accepted
        );
    }

    #[test]
    fn stale_accept_retries_after_an_invitation_race_rebases_the_cas_revision() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(24);
        let first_account = account(25);
        let package = package("stale-invite-race");
        let first_id = MusubiInviteIdV1::new([0x24; 32]);
        seed_package_owner(&package, &owner, 1, &mut transaction);

        InviteMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: first_id,
            invited_account: first_account.clone(),
            role: MusubiPackageRoleV1::Owner,
            expires_at_height: 100,
            expected_governance_revision: 1,
        }
        .execute(&owner, &mut transaction)
        .expect("first invitation succeeds");
        InviteMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: MusubiInviteIdV1::new([0x25; 32]),
            invited_account: account(26),
            role: MusubiPackageRoleV1::Owner,
            expires_at_height: 100,
            expected_governance_revision: 2,
        }
        .execute(&owner, &mut transaction)
        .expect("racing invitation advances and rebases governance");
        let _ = take_musubi_events(&mut transaction);

        let stale = AcceptMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: first_id,
            expected_governance_revision: 2,
        }
        .execute(&first_account, &mut transaction)
        .expect_err("the pre-race CAS revision must remain stale");
        assert!(
            stale
                .to_string()
                .contains("stale Musubi package governance")
        );
        let invitation = transaction
            .world
            .musubi_package_invitations
            .get(&first_id)
            .expect("stale acceptance leaves the invitation pending");
        assert_eq!(invitation.state, MusubiInvitationStateV1::Pending);
        assert_eq!(invitation.expected_governance_revision, 3);
        assert_eq!(
            transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("stale acceptance leaves the package unchanged")
                .revisions
                .governance,
            3
        );
        assert!(take_musubi_events(&mut transaction).is_empty());

        AcceptMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: first_id,
            expected_governance_revision: 3,
        }
        .execute(&first_account, &mut transaction)
        .expect("acceptance retries successfully at the rebased revision");
        assert_eq!(
            transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("retried acceptance advances governance")
                .revisions
                .governance,
            4
        );
    }

    #[test]
    fn invalid_invitation_is_rejected_before_pending_invitations_are_rebased() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(27);
        let pending_account = account(28);
        let package = package("invalid-invite-atomicity");
        let pending = MusubiMaintainerInvitationV1 {
            invite_id: MusubiInviteIdV1::new([0x28; 32]),
            package: package.clone(),
            invited_by: owner.clone(),
            invited_account: pending_account.clone(),
            role: MusubiPackageRoleV1::Owner,
            expected_governance_revision: 1,
            expires_at_height: 100,
            state: MusubiInvitationStateV1::Pending,
        };
        let pending_directory_key = MusubiMaintainerDirectoryKeyV1::pending(
            package.clone(),
            pending_account,
            pending.invite_id,
        );
        seed_package_owner(&package, &owner, 1, &mut transaction);
        seed_pending_invitation(pending.clone(), &mut transaction);
        let directory_before = transaction
            .world
            .musubi_maintainer_directory
            .iter()
            .map(|(key, entry)| (key.clone(), entry.clone()))
            .collect::<Vec<_>>();

        let error = InviteMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: MusubiInviteIdV1::new([0; 32]),
            invited_account: account(29),
            role: MusubiPackageRoleV1::Owner,
            expires_at_height: 100,
            expected_governance_revision: 1,
        }
        .execute(&owner, &mut transaction)
        .expect_err("a zero invitation identity must fail before governance advances");

        assert!(error.to_string().contains("invitation is invalid"));
        assert_eq!(
            transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("package remains")
                .revisions
                .governance,
            1
        );
        assert_eq!(
            transaction
                .world
                .musubi_package_invitations
                .get(&pending.invite_id),
            Some(&pending)
        );
        assert_eq!(
            transaction
                .world
                .musubi_maintainer_directory
                .get(&pending_directory_key),
            Some(&MusubiMaintainerDirectoryEntryV1::PendingInvitation(
                pending
            ))
        );
        assert_eq!(
            transaction
                .world
                .musubi_maintainer_directory
                .iter()
                .map(|(key, entry)| (key.clone(), entry.clone()))
                .collect::<Vec<_>>(),
            directory_before
        );
        assert!(
            transaction
                .world
                .musubi_package_invitations
                .get(&MusubiInviteIdV1::new([0; 32]))
                .is_none()
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn publication_index_overflow_drops_the_unapplied_invitation_plan() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(30);
        let invited = account(31);
        let package = package("publication-plan-overflow");
        let pending = MusubiMaintainerInvitationV1 {
            invite_id: MusubiInviteIdV1::new([0x30; 32]),
            package: package.clone(),
            invited_by: owner.clone(),
            invited_account: invited,
            role: MusubiPackageRoleV1::Owner,
            expected_governance_revision: 1,
            expires_at_height: 100,
            state: MusubiInvitationStateV1::Pending,
        };
        seed_package_owner(&package, &owner, 1, &mut transaction);
        seed_pending_invitation(pending.clone(), &mut transaction);
        *transaction.world.musubi_resolver_index_revision.get_mut() =
            crate::state::MusubiResolverIndexRevisionV1::new(u64::MAX)
                .expect("maximum resolver revision remains nonzero");
        let directory_before = transaction
            .world
            .musubi_maintainer_directory
            .iter()
            .map(|(key, entry)| (key.clone(), entry.clone()))
            .collect::<Vec<_>>();

        let error = (|| -> Result<(), Error> {
            let mut candidate = transaction
                .world
                .musubi_packages
                .get(&package)
                .cloned()
                .expect("seeded package remains");
            let advance = plan_package_governance_advance(
                &mut candidate,
                execution_height(&transaction),
                None,
                transaction.world(),
            )?;
            let planned_index_revision = plan_resolver_index_revision(&transaction)?;
            *transaction.world.musubi_resolver_index_revision.get_mut() = planned_index_revision;
            transaction
                .world
                .musubi_packages
                .insert(package.clone(), candidate);
            advance.apply_invitation_updates(&mut transaction);
            Ok(())
        })()
        .expect_err("publication must fail when the resolver revision cannot advance");

        assert!(
            error
                .to_string()
                .contains("resolver-index revision overflow")
        );
        assert_eq!(
            transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("package remains")
                .revisions
                .governance,
            1
        );
        assert_eq!(
            transaction
                .world
                .musubi_package_invitations
                .get(&pending.invite_id),
            Some(&pending)
        );
        assert_eq!(
            transaction
                .world
                .musubi_maintainer_directory
                .iter()
                .map(|(key, entry)| (key.clone(), entry.clone()))
                .collect::<Vec<_>>(),
            directory_before
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            u64::MAX
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn publication_reverse_reference_failure_drops_the_unapplied_invitation_plan() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(32);
        let package = package("publication-reverse-bound");
        let pending = MusubiMaintainerInvitationV1 {
            invite_id: MusubiInviteIdV1::new([0x32; 32]),
            package: package.clone(),
            invited_by: owner.clone(),
            invited_account: account(33),
            role: MusubiPackageRoleV1::Owner,
            expected_governance_revision: 1,
            expires_at_height: 100,
            state: MusubiInvitationStateV1::Pending,
        };
        seed_package_owner(&package, &owner, 1, &mut transaction);
        seed_pending_invitation(pending.clone(), &mut transaction);
        let archive_id = ArchiveId::new([0x34; 32]);
        let releases = (0..MUSUBI_MAX_RESOLUTION_NODES_V1)
            .map(|patch| {
                MusubiReleaseIdV1::new(
                    package.clone(),
                    MusubiVersionV1::new(
                        1,
                        0,
                        u64::try_from(patch).expect("bounded patch fits u64"),
                        Vec::new(),
                    )
                    .expect("bounded release version is valid"),
                )
            })
            .collect::<Vec<_>>();
        let references = MusubiArchiveReverseReferencesV1 {
            archive_id,
            releases,
        };
        references
            .validate()
            .expect("maximum-size reverse-reference fixture is valid");
        transaction
            .world
            .musubi_archive_reverse_references
            .insert(archive_id, references.clone());
        let directory_before = transaction
            .world
            .musubi_maintainer_directory
            .iter()
            .map(|(key, entry)| (key.clone(), entry.clone()))
            .collect::<Vec<_>>();
        let new_release = MusubiReleaseIdV1::new(
            package.clone(),
            MusubiVersionV1::new(
                1,
                0,
                u64::try_from(MUSUBI_MAX_RESOLUTION_NODES_V1).expect("bounded patch fits u64"),
                Vec::new(),
            )
            .expect("successor release version is valid"),
        );

        let error = (|| -> Result<(), Error> {
            let mut candidate = transaction
                .world
                .musubi_packages
                .get(&package)
                .cloned()
                .expect("seeded package remains");
            let advance = plan_package_governance_advance(
                &mut candidate,
                execution_height(&transaction),
                None,
                transaction.world(),
            )?;
            let planned_references = plan_archive_reverse_reference(
                archive_id,
                new_release.clone(),
                transaction.world(),
            )?;
            transaction
                .world
                .musubi_packages
                .insert(package.clone(), candidate);
            transaction
                .world
                .musubi_archive_reverse_references
                .insert(archive_id, planned_references);
            advance.apply_invitation_updates(&mut transaction);
            Ok(())
        })()
        .expect_err("publication must fail when an archive reference bound is exhausted");

        assert!(error.to_string().contains("reverse references"));
        assert_eq!(
            transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("package remains")
                .revisions
                .governance,
            1
        );
        assert_eq!(
            transaction
                .world
                .musubi_package_invitations
                .get(&pending.invite_id),
            Some(&pending)
        );
        assert_eq!(
            transaction
                .world
                .musubi_maintainer_directory
                .iter()
                .map(|(key, entry)| (key.clone(), entry.clone()))
                .collect::<Vec<_>>(),
            directory_before
        );
        assert_eq!(
            transaction
                .world
                .musubi_archive_reverse_references
                .get(&archive_id),
            Some(&references)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn package_pending_invitation_bound_is_enforced_before_mutation() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(31);
        let package = MusubiPackageIdV1::new(
            iroha_data_model::nexus::DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "bounded-invites".parse().expect("package name"),
        );
        transaction.world.musubi_packages.insert(
            package.clone(),
            MusubiPackageRecordV1 {
                package: package.clone(),
                claimed_namespace: "sora".parse().expect("namespace"),
                claimed_namespace_binding: MusubiNamespaceBindingDigestV1::new([1; 32]),
                owners: vec![owner.clone()],
                member_accounts: vec![owner.clone()],
                claimed_at_height: 1,
                revisions: MusubiPackageRevisionsV1 {
                    governance: 1,
                    metadata: 1,
                    archive_locations: 1,
                },
            },
        );
        let owner_member = MusubiPackageMemberV1 {
            package: package.clone(),
            account: owner.clone(),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 1,
            governance_revision: 1,
        };
        transaction
            .world
            .musubi_package_members
            .insert(owner_member.key(), owner_member.clone());
        upsert_maintainer_directory(
            MusubiMaintainerDirectoryEntryV1::Accepted(owner_member),
            &mut transaction,
        );

        for index in 0..MUSUBI_MAX_PENDING_INVITATIONS_V1 {
            let mut bytes = [0_u8; 32];
            bytes[..8].copy_from_slice(
                &u64::try_from(index + 1)
                    .expect("bounded fixture index fits u64")
                    .to_le_bytes(),
            );
            let invitation = MusubiMaintainerInvitationV1 {
                invite_id: MusubiInviteIdV1::new(bytes),
                package: package.clone(),
                invited_by: owner.clone(),
                invited_account: account(u8::try_from(index % 200 + 32).expect("account seed")),
                role: MusubiPackageRoleV1::Owner,
                expected_governance_revision: 1,
                expires_at_height: 100,
                state: MusubiInvitationStateV1::Pending,
            };
            upsert_maintainer_directory(
                MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation),
                &mut transaction,
            );
        }
        assert_eq!(
            pending_invitation_count(&package, transaction.world()),
            MUSUBI_MAX_PENDING_INVITATIONS_V1
        );

        let instruction = InviteMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: MusubiInviteIdV1::new([0xFE; 32]),
            invited_account: account(232),
            role: MusubiPackageRoleV1::Owner,
            expires_at_height: 100,
            expected_governance_revision: 1,
        };
        let error = instruction
            .execute(&owner, &mut transaction)
            .expect_err("the 257th pending invitation must fail closed");
        assert!(error.to_string().contains("pending-invitation bound"));
        assert_eq!(
            transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("package remains")
                .revisions
                .governance,
            1
        );
    }

    #[test]
    fn expired_pending_invitations_reclaim_bound_and_emit_bounded_events() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(10).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(31);
        let package = package("expiry-reclaim");
        seed_package_owner(&package, &owner, 1, &mut transaction);

        for index in 0..MUSUBI_MAX_PENDING_INVITATIONS_V1 {
            let mut bytes = [0_u8; 32];
            bytes[..8].copy_from_slice(
                &u64::try_from(index + 1)
                    .expect("bounded fixture index fits u64")
                    .to_le_bytes(),
            );
            seed_pending_invitation(
                MusubiMaintainerInvitationV1 {
                    invite_id: MusubiInviteIdV1::new(bytes),
                    package: package.clone(),
                    invited_by: owner.clone(),
                    invited_account: account(u8::try_from(index % 200 + 40).expect("account seed")),
                    role: MusubiPackageRoleV1::Owner,
                    expected_governance_revision: 1,
                    expires_at_height: 5,
                    state: MusubiInvitationStateV1::Pending,
                },
                &mut transaction,
            );
        }
        assert_eq!(
            pending_invitation_count(&package, transaction.world()),
            MUSUBI_MAX_PENDING_INVITATIONS_V1
        );

        let replacement_id = MusubiInviteIdV1::new([0xFE; 32]);
        InviteMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id: replacement_id,
            invited_account: account(250),
            role: MusubiPackageRoleV1::Owner,
            expires_at_height: 100,
            expected_governance_revision: 1,
        }
        .execute(&owner, &mut transaction)
        .expect("expired invitations reclaim capacity before the bound check");

        assert_eq!(
            transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("package remains")
                .revisions
                .governance,
            2
        );
        assert_eq!(pending_invitation_count(&package, transaction.world()), 1);
        let (expired, pending) = transaction.world.musubi_package_invitations.iter().fold(
            (0_usize, 0_usize),
            |(expired, pending), (_, invitation)| match invitation.state {
                MusubiInvitationStateV1::Expired => (expired + 1, pending),
                MusubiInvitationStateV1::Pending => (expired, pending + 1),
                MusubiInvitationStateV1::Accepted | MusubiInvitationStateV1::Revoked => {
                    (expired, pending)
                }
            },
        );
        assert_eq!(expired, MUSUBI_MAX_PENDING_INVITATIONS_V1);
        assert_eq!(pending, 1);
        assert_eq!(
            transaction
                .world
                .musubi_package_invitations
                .get(&replacement_id)
                .expect("replacement invitation")
                .state,
            MusubiInvitationStateV1::Pending
        );

        let events = take_musubi_events(&mut transaction);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, MusubiEvent::MaintainerInvitationExpired(_)))
                .count(),
            MUSUBI_MAX_PENDING_INVITATIONS_V1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, MusubiEvent::MaintainerInvited(_)))
                .count(),
            1
        );
        assert_eq!(events.len(), MUSUBI_MAX_PENDING_INVITATIONS_V1 + 1);
    }

    #[test]
    fn invitation_revoke_is_owner_only_cas_and_replay_safe() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(5).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(51);
        let stranger = account(52);
        let invited = account(53);
        let package = package("revoke-invite");
        let invite_id = MusubiInviteIdV1::new([0x53; 32]);
        seed_package_owner(&package, &owner, 1, &mut transaction);
        seed_pending_invitation(
            MusubiMaintainerInvitationV1 {
                invite_id,
                package: package.clone(),
                invited_by: owner.clone(),
                invited_account: invited.clone(),
                role: MusubiPackageRoleV1::Owner,
                expected_governance_revision: 1,
                expires_at_height: 100,
                state: MusubiInvitationStateV1::Pending,
            },
            &mut transaction,
        );

        let revoke = |expected_governance_revision| RevokeMusubiPackageMaintainerInvitationV1 {
            package: package.clone(),
            invite_id,
            expected_governance_revision,
        };
        let unauthorized = revoke(1)
            .execute(&stranger, &mut transaction)
            .expect_err("a non-owner cannot revoke an invitation");
        assert!(unauthorized.to_string().contains("not an owner"));
        let stale = revoke(2)
            .execute(&owner, &mut transaction)
            .expect_err("a stale governance revision fails closed");
        assert!(
            stale
                .to_string()
                .contains("stale Musubi package governance")
        );
        assert!(take_musubi_events(&mut transaction).is_empty());

        revoke(1)
            .execute(&owner, &mut transaction)
            .expect("the current owner may revoke the pending invitation");
        assert_eq!(
            transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("package remains")
                .revisions
                .governance,
            2
        );
        assert_eq!(
            transaction
                .world
                .musubi_package_invitations
                .get(&invite_id)
                .expect("historical invitation remains")
                .state,
            MusubiInvitationStateV1::Revoked
        );
        assert!(
            transaction
                .world
                .musubi_maintainer_directory
                .get(&MusubiMaintainerDirectoryKeyV1::pending(
                    package.clone(),
                    invited,
                    invite_id,
                ))
                .is_none()
        );
        assert!(matches!(
            take_musubi_events(&mut transaction).as_slice(),
            [MusubiEvent::MaintainerInvitationRevoked(_)]
        ));

        let replay = revoke(2)
            .execute(&owner, &mut transaction)
            .expect_err("a terminal invitation cannot be revoked twice");
        assert!(replay.to_string().contains("not pending"));
        let old_revision_replay = revoke(1)
            .execute(&owner, &mut transaction)
            .expect_err("the original compare-and-set revision cannot be replayed");
        assert!(
            old_revision_replay
                .to_string()
                .contains("stale Musubi package governance")
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn accepting_an_expired_invitation_fails_without_mutating_it() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(10).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(61);
        let invited = account(62);
        let package = package("expired-accept");
        let invite_id = MusubiInviteIdV1::new([0x62; 32]);
        seed_package_owner(&package, &owner, 1, &mut transaction);
        seed_pending_invitation(
            MusubiMaintainerInvitationV1 {
                invite_id,
                package: package.clone(),
                invited_by: owner,
                invited_account: invited.clone(),
                role: MusubiPackageRoleV1::Owner,
                expected_governance_revision: 1,
                expires_at_height: 9,
                state: MusubiInvitationStateV1::Pending,
            },
            &mut transaction,
        );

        let error = AcceptMusubiPackageMaintainerV1 {
            package: package.clone(),
            invite_id,
            expected_governance_revision: 1,
        }
        .execute(&invited, &mut transaction)
        .expect_err("height-expired invitations are never accepted");
        assert!(
            error
                .to_string()
                .contains("next successful package governance mutation")
        );
        assert_eq!(
            transaction
                .world
                .musubi_package_invitations
                .get(&invite_id)
                .expect("pending record remains until governance cleanup")
                .state,
            MusubiInvitationStateV1::Pending
        );
        assert_eq!(pending_invitation_count(&package, transaction.world()), 1);
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn maintainer_query_visibility_excludes_only_height_expired_invitations() {
        let package = package("query-expiry");
        let owner = account(71);
        let accepted = MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
            package: package.clone(),
            account: owner.clone(),
            role: MusubiPackageRoleV1::Owner,
            accepted_at_height: 1,
            governance_revision: 1,
        });
        let mut invitation = MusubiMaintainerInvitationV1 {
            invite_id: MusubiInviteIdV1::new([0x71; 32]),
            package,
            invited_by: owner,
            invited_account: account(72),
            role: MusubiPackageRoleV1::Owner,
            expected_governance_revision: 1,
            expires_at_height: 9,
            state: MusubiInvitationStateV1::Pending,
        };
        assert!(maintainer_directory_entry_visible_at_height(&accepted, 10));
        assert!(!maintainer_directory_entry_visible_at_height(
            &MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation.clone()),
            10,
        ));
        invitation.expires_at_height = 10;
        assert!(maintainer_directory_entry_visible_at_height(
            &MusubiMaintainerDirectoryEntryV1::PendingInvitation(invitation),
            10,
        ));
    }

    #[test]
    fn identical_alias_replay_requires_current_package_owner_authorization() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(81);
        let stranger = account(82);
        let package = package("alias-target");
        let alias: MusubiAliasNameV1 = "repeat".parse().expect("alias");
        seed_package_owner(&package, &owner, 1, &mut transaction);
        transaction.world.musubi_aliases.insert(
            alias.clone(),
            MusubiAliasRecordV1 {
                alias: alias.clone(),
                target: package.clone(),
                registered_by: owner.clone(),
                pricing_revision: 1,
                paid_xor: 1,
                registered_at_height: 1,
                history_revision: 1,
            },
        );
        let mut closed_policy = MusubiRegistryPolicyV1::default();
        closed_policy.revision = 2;
        closed_policy.mode = MusubiRegistryAdmissionModeV1::Closed;
        closed_policy.alias_pricing.revision = 2;
        closed_policy.alias_pricing.length_5_to_32_xor = 2;
        *transaction.world.musubi_registry_policy.get_mut() = closed_policy;

        let error = RegisterMusubiAliasV1 {
            alias: alias.clone(),
            target: package.clone(),
            expected_pricing_revision: 1,
        }
        .execute(&stranger, &mut transaction)
        .expect_err("an arbitrary authority cannot obtain successful alias replay");
        assert!(error.to_string().contains("not an owner"));
        assert_eq!(
            transaction
                .world
                .musubi_aliases
                .get(&alias)
                .expect("alias remains")
                .history_revision,
            1
        );
        assert!(take_musubi_events(&mut transaction).is_empty());

        RegisterMusubiAliasV1 {
            alias: alias.clone(),
            target: package,
            expected_pricing_revision: u64::MAX,
        }
        .execute(&owner, &mut transaction)
        .expect("the current owner may replay an identical alias under closed admission");
        assert_eq!(
            transaction
                .world
                .musubi_aliases
                .get(&alias)
                .expect("alias remains")
                .history_revision,
            1
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn location_reverse_indices_reject_reuse_and_retain_tombstones() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let pin = iroha_data_model::sorafs::pin_registry::ManifestDigest::new([0xA1; 32]);
        let order = iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new([0xA2; 32]);
        let first = location_fixture(0xA3, pin, order);
        bind_location_reverse_indices(None, &first, &mut transaction)
            .expect("first exact location binding succeeds");
        assert!(
            transaction
                .world
                .musubi_locations_by_pin
                .get(&pin)
                .is_some_and(|reference| reference.active && reference.location == first.key())
        );

        let conflicting = location_fixture(
            0xA4,
            pin,
            iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new([0xA5; 32]),
        );
        bind_location_reverse_indices(None, &conflicting, &mut transaction)
            .expect_err("one pin manifest cannot be rebound to another location");

        retire_location_reverse_indices(&first, &mut transaction)
            .expect("retirement atomically leaves reuse tombstones");
        assert!(
            transaction
                .world
                .musubi_locations_by_pin
                .get(&pin)
                .is_some_and(|reference| !reference.active && reference.location == first.key())
        );
        bind_location_reverse_indices(None, &conflicting, &mut transaction)
            .expect_err("retired pin tombstones permanently reject reuse");
    }

    #[test]
    fn namespace_binding_replay_requires_current_owner_authorization() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let owner = account(1);
        let stranger = account(2);
        let selector =
            crate::sns::selector_for_dataspace_alias("sora").expect("dataspace alias selector");
        let address = iroha_data_model::account::AccountAddress::from_account_id(&owner)
            .expect("account address");
        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            crate::sns::SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace id metadata key"),
            iroha_primitives::json::Json::new(7_u64),
        );
        let mut record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            10,
            20,
            30,
            metadata,
        );
        transaction
            .world
            .smart_contract_state_mut_for_testing()
            .insert(crate::sns::record_storage_key(&selector), record.encode());
        let binding = MusubiNamespaceBindingV1 {
            namespace: "sora".parse().expect("namespace"),
            home_dataspace: iroha_data_model::nexus::DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::DataspaceRoot,
            generation: 1,
        };
        transaction
            .world
            .musubi_namespace_bindings
            .insert(binding.namespace.clone(), binding.clone());
        let mut closed_policy = MusubiRegistryPolicyV1::default();
        closed_policy.revision = 2;
        closed_policy.mode = MusubiRegistryAdmissionModeV1::Closed;
        *transaction.world.musubi_registry_policy.get_mut() = closed_policy;

        let unauthorized = RegisterMusubiNamespaceBindingV1::new(binding.clone(), 1)
            .execute(&stranger, &mut transaction)
            .expect_err("an arbitrary authority cannot obtain a successful namespace replay");
        assert!(unauthorized.to_string().contains("does not own"));
        assert!(transaction.world.take_external_events().is_empty());

        RegisterMusubiNamespaceBindingV1::new(binding.clone(), u64::MAX)
            .execute(&owner, &mut transaction)
            .expect("the current owner may replay an identical binding under closed admission");
        assert!(transaction.world.take_external_events().is_empty());

        record.owner = stranger.clone();
        record.ownership_generation = 2;
        transaction
            .world
            .smart_contract_state_mut_for_testing()
            .insert(crate::sns::record_storage_key(&selector), record.encode());
        let former_owner = RegisterMusubiNamespaceBindingV1::new(binding.clone(), u64::MAX)
            .execute(&owner, &mut transaction)
            .expect_err("the former namespace owner cannot replay after ownership changes");
        assert!(former_owner.to_string().contains("does not own"));
        RegisterMusubiNamespaceBindingV1::new(binding.clone(), u64::MAX)
            .execute(&stranger, &mut transaction)
            .expect("the live owner may replay an immutable older-generation binding");
        assert!(transaction.world.take_external_events().is_empty());

        let conflicting = MusubiNamespaceBindingV1 {
            home_dataspace: iroha_data_model::nexus::DataSpaceId::new(8),
            ..binding
        };
        RegisterMusubiNamespaceBindingV1::new(conflicting, 1)
            .execute(&stranger, &mut transaction)
            .expect_err("conflicting immutable namespace binding is rejected");
        assert!(transaction.world.take_external_events().is_empty());
    }

    #[test]
    fn namespace_claim_uses_live_owner_generation_after_immutable_binding_registration() {
        let owner_keypair = KeyPair::try_from_seed(vec![41; 32], Algorithm::Ed25519)
            .expect("owner fixture keypair");
        let owner = AccountId::new(owner_keypair.public_key().clone());
        let delegate = account(42);
        let binding = MusubiNamespaceBindingV1 {
            namespace: "dex.universal".parse().expect("namespace"),
            home_dataspace: iroha_data_model::nexus::DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
            generation: 1,
        };
        let sign_delegation = |owner_generation| {
            let payload = MusubiNamespaceDelegationPayloadV1 {
                version: MUSUBI_REGISTRY_VERSION_V1,
                namespace_binding: binding.digest(),
                owner_generation,
                owner: owner.clone(),
                delegate: delegate.clone(),
                expires_at_height: 100,
            };
            MusubiNamespaceDelegationV1 {
                approvals: vec![MusubiNamespaceDelegationApprovalV1 {
                    public_key: owner_keypair.public_key().clone(),
                    signature: SignatureOf::try_from_hash(
                        owner_keypair.private_key(),
                        payload.signing_hash(),
                    )
                    .expect("sign namespace delegation"),
                }],
                payload,
            }
        };

        validate_namespace_claim_authority(&binding, None, &owner, &owner, 2, 50)
            .expect("the live owner may claim after ownership generation advances");
        validate_namespace_claim_authority(
            &binding,
            Some(&sign_delegation(2)),
            &delegate,
            &owner,
            2,
            50,
        )
        .expect("a delegation signed by the live owner generation may claim");
        validate_namespace_claim_authority(
            &binding,
            Some(&sign_delegation(1)),
            &delegate,
            &owner,
            2,
            50,
        )
        .expect_err("a delegation from the immutable binding generation is stale");
        validate_namespace_claim_authority(&binding, None, &owner, &owner, 0, 50)
            .expect_err("a zero live ownership generation must fail closed");
    }

    #[test]
    fn namespace_home_dataspace_matches_catalog_for_root_and_domain_scopes() {
        let world = World::default();
        let catalog = iroha_data_model::nexus::DataSpaceCatalog::default();
        let bindings = [
            MusubiNamespaceBindingV1 {
                namespace: "universal".parse().expect("root namespace"),
                home_dataspace: iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
                scope: MusubiPackageScopeV1::DataspaceRoot,
                generation: 1,
            },
            MusubiNamespaceBindingV1 {
                namespace: "dex.universal".parse().expect("domain namespace"),
                home_dataspace: iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
                scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
                generation: 1,
            },
        ];

        for binding in &bindings {
            validate_namespace_home_dataspace(binding, &world.view(), &catalog, 50)
                .expect("namespace alias and structural dataspace agree");
            let mismatched = MusubiNamespaceBindingV1 {
                home_dataspace: iroha_data_model::nexus::DataSpaceId::new(7),
                ..binding.clone()
            };
            validate_namespace_home_dataspace(&mismatched, &world.view(), &catalog, 50)
                .expect_err("cross-dataspace namespace binding must fail closed");
        }
    }

    #[test]
    fn namespace_home_dataspace_rejects_static_dynamic_alias_conflicts_for_all_scopes() {
        let catalog = iroha_data_model::nexus::DataSpaceCatalog::default();
        let selector =
            crate::sns::selector_for_dataspace_alias("universal").expect("dataspace selector");
        let owner = account(43);
        let address = iroha_data_model::account::AccountAddress::from_account_id(&owner)
            .expect("account address");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            owner,
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            iroha_data_model::metadata::Metadata::default(),
        );
        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(crate::sns::record_storage_key(&selector), record.encode());
        let bindings = [
            MusubiNamespaceBindingV1 {
                namespace: "universal".parse().expect("root namespace"),
                home_dataspace: iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
                scope: MusubiPackageScopeV1::DataspaceRoot,
                generation: 1,
            },
            MusubiNamespaceBindingV1 {
                namespace: "dex.universal".parse().expect("domain namespace"),
                home_dataspace: iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
                scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
                generation: 1,
            },
        ];

        for binding in &bindings {
            let error = validate_namespace_home_dataspace(binding, &world.view(), &catalog, 50)
                .expect_err("conflicting static and dynamic dataspace mappings must fail closed");
            assert!(
                error
                    .to_string()
                    .contains(crate::sns::ALIAS_CATALOG_MAPPING_CONFLICT_CODE),
                "unexpected namespace mapping error: {error}"
            );
        }
    }

    #[test]
    fn release_yank_rejects_decoded_empty_reason_before_state_lookup() {
        let release = MusubiReleaseIdV1::new(
            MusubiPackageIdV1::new(
                iroha_data_model::nexus::DataSpaceId::new(7),
                MusubiPackageScopeV1::DataspaceRoot,
                "validation".parse().expect("package name"),
            ),
            "1.0.0".parse().expect("release version"),
        );
        let canonical = SetMusubiReleaseYankV1::new(
            release,
            true,
            MusubiReasonV1::new("valid reason").expect("reason"),
            1,
        );
        let json = norito::json::to_json(&canonical).expect("serialize yank request");
        let hostile = json.replacen("valid reason", "", 1);
        assert_ne!(hostile, json, "reason fixture must be replaced");
        let decoded: SetMusubiReleaseYankV1 =
            norito::json::from_json(&hostile).expect("decode structurally valid hostile request");

        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("nonzero block height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let error = decoded
            .execute(&account(44), &mut transaction)
            .expect_err("decoded empty reason must fail before the missing-release lookup");
        assert!(
            matches!(error, Error::InvalidParameter(_)),
            "unexpected decoded-yank rejection: {error}"
        );
    }

    #[test]
    fn parliament_consumption_records_server_execution_height() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(GOVERNANCE_EXECUTION_HEIGHT)
                .expect("nonzero governance fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let minimum_enactment_delay = transaction.gov.min_enactment_delay;
        let decision = MusubiGovernanceDecisionV1 {
            decision_id: [0x91; 32],
            action_digest: MusubiGovernanceActionDigestV1::new([0x92; 32]),
            enacted_at_height: GOVERNANCE_EXECUTION_HEIGHT
                .checked_sub(minimum_enactment_delay.max(1))
                .expect("governance fixture height exceeds its minimum delay"),
            execute_after_height: GOVERNANCE_EXECUTION_HEIGHT,
        };

        consume_parliament_decision(decision, &mut transaction)
            .expect("valid decision is consumed at the server block height");
        let consumed = transaction
            .world
            .musubi_governance_decisions
            .get(&decision.decision_id)
            .expect("decision consumption retained");
        assert_eq!(consumed.decision, decision);
        assert_eq!(consumed.minimum_enactment_delay, minimum_enactment_delay);
        assert_eq!(consumed.consumed_at_height, GOVERNANCE_EXECUTION_HEIGHT);
    }

    #[test]
    fn proposal_fingerprint_mismatch_is_rejected_before_recovery_mutation() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(GOVERNANCE_EXECUTION_HEIGHT)
                .expect("nonzero governance fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let package = package("fingerprint-guard");
        let old_owner = account(81);
        let replacement = account(82);
        seed_package_owner(&package, &old_owner, 1, &mut transaction);
        let action = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: package.clone(),
            owners: vec![replacement.clone()],
            expected_revision: 1,
        });
        let wrong_id = [0xA5; 32];
        let kind = ProposalKind::MusubiRegistryGovernance(action.clone());
        assert_ne!(wrong_id, kind.fingerprint());
        let decision = decision_for_current_block(wrong_id, &action, &transaction);
        insert_enacted_proposal(wrong_id, kind, decision.enacted_at_height, &mut transaction);

        let error = RecoverMusubiPackageV1 {
            decision,
            package: package.clone(),
            owners: vec![replacement.clone()],
            expected_governance_revision: 1,
        }
        .execute(&account(83), &mut transaction)
        .expect_err("a proposal stored under a non-fingerprint key must fail closed");
        assert!(error.to_string().contains("fingerprint"), "{error}");

        let persisted = transaction
            .world
            .musubi_packages
            .get(&package)
            .expect("seeded package remains");
        assert_eq!(persisted.owners, vec![old_owner]);
        assert_eq!(persisted.revisions.governance, 1);
        assert!(
            transaction
                .world
                .musubi_package_members
                .get(&MusubiPackageMemberKeyV1::new(package, replacement))
                .is_none()
        );
        assert!(
            transaction
                .world
                .musubi_governance_decisions
                .get(&wrong_id)
                .is_none()
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn owner_recovery_binds_consumption_state_event_and_rejects_replay() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(GOVERNANCE_EXECUTION_HEIGHT)
                .expect("nonzero governance fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let package = package("recovery-proof");
        let old_owner = account(84);
        seed_package_owner(&package, &old_owner, 1, &mut transaction);
        let mut replacement_owners = vec![account(86), account(85)];
        replacement_owners.sort();
        replacement_owners.dedup();
        let action = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: package.clone(),
            owners: replacement_owners.clone(),
            expected_revision: 1,
        });
        let decision = seed_enacted_decision(&action, &mut transaction);

        RecoverMusubiPackageV1 {
            decision,
            package: package.clone(),
            owners: replacement_owners.clone(),
            expected_governance_revision: 1,
        }
        .execute(&account(87), &mut transaction)
        .expect("canonical owner recovery executes");

        let consumption = *transaction
            .world
            .musubi_governance_decisions
            .get(&decision.decision_id)
            .expect("decision consumption retained");
        assert_eq!(consumption.decision, decision);
        assert_eq!(consumption.consumed_at_height, GOVERNANCE_EXECUTION_HEIGHT);
        let persisted = transaction
            .world
            .musubi_packages
            .get(&package)
            .cloned()
            .expect("recovered package retained");
        assert_eq!(persisted.owners, replacement_owners);
        assert_eq!(persisted.member_accounts, replacement_owners);
        assert_eq!(persisted.revisions.governance, 2);
        for owner in &replacement_owners {
            let member = transaction
                .world
                .musubi_package_members
                .get(&MusubiPackageMemberKeyV1::new(
                    package.clone(),
                    owner.clone(),
                ))
                .expect("recovered owner member retained");
            assert_eq!(member.role, MusubiPackageRoleV1::Owner);
            assert_eq!(member.governance_revision, 2);
            assert_eq!(member.accepted_at_height, consumption.consumed_at_height);
        }
        assert!(
            transaction
                .world
                .musubi_package_members
                .get(&MusubiPackageMemberKeyV1::new(package.clone(), old_owner))
                .is_none()
        );

        let events = take_musubi_events(&mut transaction);
        let [MusubiEvent::PackageRecovered(event)] = events.as_slice() else {
            panic!("expected exactly one package-recovery event: {events:?}");
        };
        assert_eq!(event.package, package);
        assert_eq!(event.action_digest, consumption.decision.action_digest);
        assert_eq!(event.finalized_height, consumption.consumed_at_height);
        assert_eq!(event.governance_revision, persisted.revisions.governance);
        assert_eq!(usize::from(event.owner_count), persisted.owners.len());

        let replay_error = RecoverMusubiPackageV1 {
            decision,
            package: package.clone(),
            owners: replacement_owners,
            expected_governance_revision: 1,
        }
        .execute(&account(87), &mut transaction)
        .expect_err("the same Parliament decision cannot be replayed");
        assert!(replay_error.to_string().contains("already consumed"));
        assert_eq!(
            transaction.world.musubi_packages.get(&package),
            Some(&persisted)
        );
        assert_eq!(
            transaction
                .world
                .musubi_governance_decisions
                .get(&decision.decision_id),
            Some(&consumption)
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn artifact_takedown_binds_state_resolver_directory_consumption_event_and_rejects_replay() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(GOVERNANCE_EXECUTION_HEIGHT)
                .expect("nonzero governance fixture height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut transaction = block.transaction();

        let archive = retention_archive(91);
        let archive_id = archive.archive_id;
        let source_digest = archive.commitment.source_tree_digest;
        let initial_release = retention_release(
            archive_id,
            "1.0.0",
            false,
            MusubiArtifactGovernanceStateV1::Available,
        );
        initial_release
            .validate()
            .expect("initial release fixture is valid");
        let release_id = initial_release.manifest.release.clone();
        let package = release_id.package.clone();
        seed_package_owner(&package, &account(92), 1, &mut transaction);

        let initial_index_revision = transaction.world.musubi_resolver_index_revision.get().get();
        let expected_index_revision = initial_index_revision
            .checked_add(1)
            .expect("fixture resolver revision has a successor");
        let initial_row = MusubiResolverReleaseRowV1 {
            release: release_id.clone(),
            release_digest: initial_release.release_digest,
            archive_id,
            source_digest,
            interface_digest: initial_release.manifest.interface_digest,
            abi: initial_release.manifest.abi,
            dependencies: initial_release.manifest.dependencies.clone(),
            selection: MusubiReleaseSelectionStateV1 {
                yank: initial_release.yank.clone(),
                storage: MusubiArchiveAvailabilityV1 {
                    archive_id,
                    availability: MusubiStorageAvailabilityV1::Selectable,
                    healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
                    active_locations: 1,
                    finalized_height: 1,
                    finalized_block_hash: [0xB1; 32],
                    index_revision: initial_index_revision,
                },
                governance: MusubiArtifactGovernanceStateV1::Available,
            },
            index_revision: initial_index_revision,
        };
        initial_row
            .validate()
            .expect("initial resolver row is fresh-selectable");
        transaction
            .world
            .musubi_releases
            .insert(release_id.clone(), initial_release.clone());
        transaction
            .world
            .musubi_resolver_index
            .insert(release_id.clone(), initial_row.clone());

        let (namespace, metadata_revision) = {
            let package_record = transaction
                .world
                .musubi_packages
                .get(&package)
                .expect("seeded package remains available");
            (
                package_record.claimed_namespace.clone(),
                package_record.revisions.metadata,
            )
        };
        let selector = MusubiPackageSelectorV1 {
            namespace,
            name: package.name.clone(),
        };
        let initial_directory = MusubiOrderedPackageEntryV1 {
            selector: selector.clone(),
            package: package.clone(),
            latest_selectable: Some(release_id.version.clone()),
            metadata_revision,
            index_revision: initial_index_revision,
        };
        initial_directory
            .validate()
            .expect("initial directory fixture is valid");
        transaction
            .world
            .musubi_public_directory
            .insert(selector.clone(), initial_directory);

        let reason: MusubiReasonV1 = "governed security response"
            .parse()
            .expect("bounded takedown reason");
        let action = MusubiParliamentActionV1::TakedownArtifact(MusubiTakedownArtifactActionV1 {
            release: release_id.clone(),
            reason: reason.clone(),
            expected_artifact_governance_revision: 1,
        });
        let decision = seed_enacted_decision(&action, &mut transaction);
        let minimum_enactment_delay = transaction.gov.min_enactment_delay;

        SetMusubiArtifactTakedownV1 {
            decision,
            release: release_id.clone(),
            reason: reason.clone(),
            expected_artifact_governance_revision: 1,
        }
        .execute(&account(93), &mut transaction)
        .expect("canonical artifact takedown executes");

        let expected_governance =
            MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
                action_digest: decision.action_digest,
                reason: reason.clone(),
                applied_at_height: GOVERNANCE_EXECUTION_HEIGHT,
            });
        let mut expected_release = initial_release;
        expected_release.artifact_governance = expected_governance.clone();
        expected_release.revisions.artifact_governance = 2;
        let mut expected_row = initial_row;
        expected_row.selection.governance = expected_governance;
        expected_row.index_revision = expected_index_revision;
        let expected_directory = MusubiOrderedPackageEntryV1 {
            selector: selector.clone(),
            package: package.clone(),
            latest_selectable: None,
            metadata_revision,
            index_revision: expected_index_revision,
        };
        let expected_consumption = MusubiGovernanceDecisionConsumptionV1 {
            decision,
            minimum_enactment_delay,
            consumed_at_height: GOVERNANCE_EXECUTION_HEIGHT,
        };
        assert!(decision.enacted_at_height < decision.execute_after_height);
        assert!(decision.execute_after_height <= expected_consumption.consumed_at_height);
        assert_eq!(
            transaction.world.musubi_releases.get(&release_id),
            Some(&expected_release)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index.get(&release_id),
            Some(&expected_row)
        );
        assert_eq!(
            transaction.world.musubi_public_directory.get(&selector),
            Some(&expected_directory)
        );
        assert_eq!(
            transaction
                .world
                .musubi_governance_decisions
                .get(&decision.decision_id),
            Some(&expected_consumption)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            expected_index_revision
        );
        assert_eq!(
            take_musubi_events(&mut transaction),
            vec![MusubiEvent::ArtifactTakenDown(
                MusubiArtifactTakedownEventV1 {
                    release: release_id.clone(),
                    archive_id,
                    action_digest: decision.action_digest,
                    governance_revision: 2,
                    finalized_height: GOVERNANCE_EXECUTION_HEIGHT,
                }
            )]
        );

        let replay_error = SetMusubiArtifactTakedownV1 {
            decision,
            release: release_id.clone(),
            reason,
            expected_artifact_governance_revision: 1,
        }
        .execute(&account(93), &mut transaction)
        .expect_err("the same artifact takedown decision cannot be replayed");
        assert!(replay_error.to_string().contains("already consumed"));
        assert_eq!(
            transaction.world.musubi_releases.get(&release_id),
            Some(&expected_release)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index.get(&release_id),
            Some(&expected_row)
        );
        assert_eq!(
            transaction.world.musubi_public_directory.get(&selector),
            Some(&expected_directory)
        );
        assert_eq!(
            transaction
                .world
                .musubi_governance_decisions
                .get(&decision.decision_id),
            Some(&expected_consumption)
        );
        assert_eq!(
            transaction.world.musubi_resolver_index_revision.get().get(),
            expected_index_revision
        );
        assert!(take_musubi_events(&mut transaction).is_empty());
    }

    #[test]
    fn empty_maintainer_role_and_revision_overflow_fail_closed() {
        let empty = MusubiMaintainerPermissionsV1 {
            publish: false,
            yank: false,
            metadata: false,
            archive_locations: false,
        };
        assert!(validate_role(MusubiPackageRoleV1::Maintainer(empty)).is_err());
        assert!(next_revision(u64::MAX, "test").is_err());
    }
}
