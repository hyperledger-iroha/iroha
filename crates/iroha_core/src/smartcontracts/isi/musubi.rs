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
    state::{GovernanceProposalStatus, StateReadOnly, StateTransaction, WorldReadOnly},
};

impl Execute for RegisterMusubiNamespaceBindingV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.binding
            .validate()
            .map_err(|error| invalid_parameter(error.reason()))?;
        if let Some(existing) = state_transaction
            .world
            .musubi_namespace_bindings
            .get(&self.binding.namespace)
        {
            return if existing == &self.binding {
                Ok(())
            } else {
                Err(invariant(format!(
                    "Musubi namespace '{}' is already bound",
                    self.binding.namespace
                )))
            };
        }
        let policy = state_transaction.world.musubi_registry_policy.get().clone();
        ensure_policy_revision(&policy, self.expected_policy_revision)?;
        ensure_admitted(
            &policy,
            Some(self.binding.home_dataspace),
            authority,
            state_transaction.world(),
        )?;
        ensure_namespace_owner(&self.binding, authority, state_transaction)?;
        let event = MusubiEvent::NamespaceBound(self.binding.clone());
        state_transaction
            .world
            .musubi_namespace_bindings
            .insert(self.binding.namespace.clone(), self.binding);
        emit_musubi_event(event, state_transaction);
        Ok(())
    }
}

impl Execute for RegisterMusubiArchiveV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.commitment
            .validate()
            .map_err(|error| invalid_parameter(error.reason()))?;
        self.staging_receipt
            .validate()
            .map_err(|error| invalid_parameter(error.reason()))?;
        let archive_id = self.commitment.archive_id();
        if let Some(existing) = state_transaction.world.musubi_archives.get(&archive_id) {
            return if existing.commitment == self.commitment
                && existing.staging_receipt == self.staging_receipt
            {
                Ok(())
            } else {
                Err(invariant(format!(
                    "Musubi archive '{}' is already registered with different commitments",
                    digest_label(archive_id.as_bytes())
                )))
            };
        }
        let policy = state_transaction.world.musubi_registry_policy.get().clone();
        ensure_policy_revision(&policy, self.expected_policy_revision)?;
        ensure_admitted(&policy, None, authority, state_transaction.world())?;
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
    }
}

impl Execute for AddMusubiArchiveLocationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if self.renew_after_epoch >= self.expires_at_epoch {
            return Err(invalid_parameter(
                "Musubi archive location renewal must precede expiry",
            ));
        }
        let mut archive = state_transaction
            .world
            .musubi_archives
            .get(&self.archive_id)
            .cloned()
            .ok_or_else(|| archive_not_found(self.archive_id))?;
        ensure_archive_manager(&archive, authority, state_transaction.world())?;
        ensure_revision(
            "archive location",
            archive.location_revision,
            self.expected_location_revision,
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
            Some(_) => {
                if archive
                    .location_ids
                    .binary_search(&self.location_id)
                    .is_err()
                {
                    return Err(invariant(
                        "Musubi archive location directory is inconsistent",
                    ));
                }
            }
            None => {
                if archive.location_ids.len() >= MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 {
                    return Err(invariant("Musubi archive location bound is exhausted"));
                }
                archive.location_ids.push(self.location_id);
                archive.location_ids.sort();
                archive.location_ids.dedup();
            }
        }
        if archive.location_ids.len() > MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 {
            return Err(invariant("Musubi archive location bound is exhausted"));
        }
        let providers = validate_sorafs_location(
            &archive,
            &self.pin_manifest,
            &self.replication_order,
            &self.provider_attestations,
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
            provider_attestations: self.provider_attestations,
            renew_after_epoch: self.renew_after_epoch,
            expires_at_epoch: self.expires_at_epoch,
            finalized_height: execution_height(state_transaction),
            revision: next_revision,
            state,
        };
        location
            .validate()
            .map_err(|error| invalid_parameter(error.reason()))?;
        let location_event = MusubiArchiveLocationEventV1 {
            location: key,
            pin_manifest: location.pin_manifest,
            replication_order: location.replication_order,
            transition: if existing_location.is_some() {
                MusubiArchiveLocationTransitionV1::Renewed
            } else {
                MusubiArchiveLocationTransitionV1::Added
            },
            state: location.state,
            revision: location.revision,
            finalized_height: location.finalized_height,
        };
        bind_location_reverse_indices(existing_location.as_ref(), &location, state_transaction)?;
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
    }
}

impl Execute for RetireMusubiArchiveLocationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut archive = state_transaction
            .world
            .musubi_archives
            .get(&self.archive_id)
            .cloned()
            .ok_or_else(|| archive_not_found(self.archive_id))?;
        ensure_archive_manager(&archive, authority, state_transaction.world())?;
        ensure_revision(
            "archive location",
            archive.location_revision,
            self.expected_location_revision,
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
    }
}

impl Execute for PublishMusubiReleaseV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.namespace
            .validate()
            .map_err(|error| invalid_parameter(error.reason()))?;
        self.publication
            .validate()
            .map_err(|error| invalid_parameter(error.reason()))?;
        let release_id = self.publication.manifest.release.clone();
        let release_digest = self.publication.manifest.release_digest();
        if let Some(existing) = state_transaction.world.musubi_releases.get(&release_id) {
            return if existing.release_digest == release_digest
                && existing.manifest == self.publication.manifest
                && state_transaction
                    .world
                    .musubi_packages
                    .get(&release_id.package)
                    .is_some_and(|package| package.claimed_namespace == self.namespace)
            {
                Ok(())
            } else {
                Err(invariant(format!(
                    "Musubi release '{release_id}' is permanently bound to different commitments"
                )))
            };
        }
        let policy = state_transaction.world.musubi_registry_policy.get().clone();
        ensure_policy_revision(&policy, self.expected_policy_revision)?;
        ensure_admitted(
            &policy,
            Some(release_id.package.home_dataspace),
            authority,
            state_transaction.world(),
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
        let package = if let Some(mut package) = existing_package {
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
            ensure_revision("package governance", package.revisions.governance, expected)?;
            ensure_package_capability(
                &release_id.package,
                authority,
                PackageCapability::Publish,
                state_transaction.world(),
            )?;
            package.revisions.governance =
                next_revision(package.revisions.governance, "package governance")?;
            package
        } else {
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
            state_transaction.world.musubi_package_members.insert(
                MusubiPackageMemberKeyV1::new(release_id.package.clone(), authority.clone()),
                MusubiPackageMemberV1 {
                    package: release_id.package.clone(),
                    account: authority.clone(),
                    role: MusubiPackageRoleV1::Owner,
                    accepted_at_height: height,
                    governance_revision: 1,
                },
            );
            state_transaction.world.musubi_package_metadata.insert(
                release_id.package.clone(),
                MusubiPackageMetadataRecordV1 {
                    package: release_id.package.clone(),
                    metadata: self.publication.manifest.metadata.clone(),
                    revision: 1,
                    changed_by: authority.clone(),
                    changed_at_height: height,
                },
            );
            package
        };
        package
            .validate()
            .map_err(|error| invariant(error.reason()))?;

        let initial_reason =
            MusubiReasonV1::new("initial publication").expect("static Musubi reason is valid");
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

        let index_revision = bump_resolver_index_revision(state_transaction)?;
        state_transaction
            .world
            .musubi_packages
            .insert(release_id.package.clone(), package);
        state_transaction
            .world
            .musubi_releases
            .insert(release_id.clone(), record);
        add_archive_reverse_reference(archive.archive_id, release_id.clone(), state_transaction)?;
        state_transaction.world.musubi_resolver_index.insert(
            release_id.clone(),
            MusubiResolverReleaseRowV1 {
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
            },
        );
        refresh_directory_for_package(&release_id.package, index_revision, state_transaction)?;
        if first_claim {
            let package = state_transaction
                .world
                .musubi_packages
                .get(&release_id.package)
                .ok_or_else(|| invariant("new Musubi package claim disappeared"))?;
            emit_musubi_event(
                MusubiEvent::PackageClaimed(MusubiPackageClaimedEventV1 {
                    package: release_id.package.clone(),
                    namespace: package.claimed_namespace.clone(),
                    claimed_by: authority.clone(),
                    governance_revision: package.revisions.governance,
                    finalized_height: height,
                }),
                state_transaction,
            );
            let metadata = state_transaction
                .world
                .musubi_package_metadata
                .get(&release_id.package)
                .cloned()
                .ok_or_else(|| invariant("new Musubi package metadata disappeared"))?;
            emit_musubi_event(
                MusubiEvent::PackageMetadataChanged(metadata),
                state_transaction,
            );
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
    }
}

impl Execute for SetMusubiReleaseYankV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
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
        )?;
        ensure_revision(
            "release yank",
            release.revisions.yank,
            self.expected_yank_revision,
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
        refresh_directory_for_package(&self.release.package, index_revision, state_transaction)?;
        emit_musubi_event(event, state_transaction);
        Ok(())
    }
}

impl Execute for SetMusubiPackageMetadataV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
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
        )?;
        ensure_revision(
            "package metadata",
            package.revisions.metadata,
            self.expected_metadata_revision,
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
    }
}

impl Execute for InviteMusubiPackageMaintainerV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        validate_role(self.role)?;
        ensure_package_owner(&self.package, authority, state_transaction.world())?;
        let mut package = state_transaction
            .world
            .musubi_packages
            .get(&self.package)
            .cloned()
            .ok_or_else(|| package_not_found(&self.package))?;
        ensure_revision(
            "package governance",
            package.revisions.governance,
            self.expected_governance_revision,
        )?;
        if self.expires_at_height <= execution_height(state_transaction) {
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
        let next = next_revision(package.revisions.governance, "package governance")?;
        package.revisions.governance = next;
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
        let event = MusubiEvent::MaintainerInvited(invitation.clone());
        state_transaction
            .world
            .musubi_packages
            .insert(self.package, package);
        state_transaction
            .world
            .musubi_package_invitations
            .insert(self.invite_id, invitation);
        emit_musubi_event(event, state_transaction);
        Ok(())
    }
}

impl Execute for AcceptMusubiPackageMaintainerV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut invitation = state_transaction
            .world
            .musubi_package_invitations
            .get(&self.invite_id)
            .cloned()
            .ok_or_else(|| invariant("Musubi package invitation was not found"))?;
        if invitation.package != self.package
            || invitation.invited_account != *authority
            || invitation.state != MusubiInvitationStateV1::Pending
        {
            return Err(invariant(
                "Musubi package invitation is not pending for this authority",
            ));
        }
        if execution_height(state_transaction) > invitation.expires_at_height {
            invitation.state = MusubiInvitationStateV1::Expired;
            state_transaction
                .world
                .musubi_package_invitations
                .insert(self.invite_id, invitation);
            return Err(invariant("Musubi package invitation has expired"));
        }
        let mut package = state_transaction
            .world
            .musubi_packages
            .get(&self.package)
            .cloned()
            .ok_or_else(|| package_not_found(&self.package))?;
        ensure_revision(
            "package governance",
            package.revisions.governance,
            self.expected_governance_revision,
        )?;
        ensure_revision(
            "package invitation",
            invitation.expected_governance_revision,
            self.expected_governance_revision,
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
        let next = next_revision(package.revisions.governance, "package governance")?;
        package.revisions.governance = next;
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
        invitation.state = MusubiInvitationStateV1::Accepted;
        invitation.expected_governance_revision = next;
        let member = MusubiPackageMemberV1 {
            package: self.package.clone(),
            account: authority.clone(),
            role: invitation.role,
            accepted_at_height: execution_height(state_transaction),
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
        state_transaction
            .world
            .musubi_package_invitations
            .insert(self.invite_id, invitation);
        emit_musubi_event(MusubiEvent::MaintainerAccepted(member), state_transaction);
        Ok(())
    }
}

impl Execute for SetMusubiPackageMaintainerRoleV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        validate_role(self.role)?;
        ensure_package_owner(&self.package, authority, state_transaction.world())?;
        let mut package = state_transaction
            .world
            .musubi_packages
            .get(&self.package)
            .cloned()
            .ok_or_else(|| package_not_found(&self.package))?;
        ensure_revision(
            "package governance",
            package.revisions.governance,
            self.expected_governance_revision,
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
            return Err(invariant("Musubi package must retain its last owner"));
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
        let next = next_revision(package.revisions.governance, "package governance")?;
        package.revisions.governance = next;
        package.owners.retain(|owner| owner != &self.account);
        if self.role == MusubiPackageRoleV1::Owner {
            package.owners.push(self.account.clone());
            package.owners.sort();
            package.owners.dedup();
        }
        package
            .validate()
            .map_err(|error| invariant(error.reason()))?;
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
            .insert(key, member);
        emit_musubi_event(event, state_transaction);
        Ok(())
    }
}

impl Execute for RemoveMusubiPackageMaintainerV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_package_owner(&self.package, authority, state_transaction.world())?;
        let mut package = state_transaction
            .world
            .musubi_packages
            .get(&self.package)
            .cloned()
            .ok_or_else(|| package_not_found(&self.package))?;
        ensure_revision(
            "package governance",
            package.revisions.governance,
            self.expected_governance_revision,
        )?;
        let key = MusubiPackageMemberKeyV1::new(self.package.clone(), self.account.clone());
        let member = state_transaction
            .world
            .musubi_package_members
            .get(&key)
            .cloned()
            .ok_or_else(|| invariant("Musubi package member was not found"))?;
        if member.role == MusubiPackageRoleV1::Owner && package.owners.len() == 1 {
            return Err(invariant("Musubi package must retain its last owner"));
        }
        package.owners.retain(|owner| owner != &self.account);
        package
            .member_accounts
            .retain(|account| account != &self.account);
        package.revisions.governance =
            next_revision(package.revisions.governance, "package governance")?;
        package
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        let event = MusubiEvent::MaintainerRemoved(MusubiPackageMemberRemovedEventV1 {
            package: self.package.clone(),
            account: self.account.clone(),
            previous_role: member.role,
            governance_revision: package.revisions.governance,
            finalized_height: execution_height(state_transaction),
        });
        state_transaction
            .world
            .musubi_packages
            .insert(self.package, package);
        state_transaction.world.musubi_package_members.remove(key);
        emit_musubi_event(event, state_transaction);
        Ok(())
    }
}

impl Execute for RegisterMusubiAliasV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.alias
            .validate()
            .map_err(|error| invalid_parameter(error.reason()))?;
        if let Some(existing) = state_transaction.world.musubi_aliases.get(&self.alias) {
            return if existing.target == self.target {
                Ok(())
            } else {
                Err(invariant(format!(
                    "Musubi alias '{}' is permanent and already registered",
                    self.alias
                )))
            };
        }
        let policy = state_transaction.world.musubi_registry_policy.get().clone();
        ensure_admitted(
            &policy,
            Some(self.target.home_dataspace),
            authority,
            state_transaction.world(),
        )?;
        if policy.alias_pricing.revision != self.expected_pricing_revision {
            return Err(stale_revision(
                "alias pricing",
                policy.alias_pricing.revision,
                self.expected_pricing_revision,
            ));
        }
        ensure_package_owner(&self.target, authority, state_transaction.world())?;
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
        crate::smartcontracts::isi::asset::isi::execute_user_numeric_asset_transfer(
            state_transaction,
            authority,
            source,
            treasury,
            Quantity::from(price),
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
    }
}

impl Execute for RecoverMusubiPackageV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let action = MusubiParliamentActionV1::RecoverPackageOwners(MusubiRecoverPackageOwnersV1 {
            package: self.package.clone(),
            owners: self.owners.clone(),
            expected_revision: self.expected_governance_revision,
        });
        verify_parliament_decision(&self.decision, &action, state_transaction)?;
        let mut package = state_transaction
            .world
            .musubi_packages
            .get(&self.package)
            .cloned()
            .ok_or_else(|| package_not_found(&self.package))?;
        ensure_revision(
            "package governance",
            package.revisions.governance,
            self.expected_governance_revision,
        )?;
        let next = next_revision(package.revisions.governance, "package governance")?;
        let existing_members = package
            .member_accounts
            .iter()
            .map(|account| {
                let key = MusubiPackageMemberKeyV1::new(self.package.clone(), account.clone());
                let member = state_transaction
                    .world
                    .musubi_package_members
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| invariant("Musubi package member directory is inconsistent"))?;
                Ok((key, member))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        for (key, member) in existing_members {
            if member.role == MusubiPackageRoleV1::Owner && !self.owners.contains(&member.account) {
                package
                    .member_accounts
                    .retain(|account| account != &member.account);
                state_transaction.world.musubi_package_members.remove(key);
            }
        }
        for owner in &self.owners {
            if package.member_accounts.binary_search(owner).is_err() {
                package.member_accounts.push(owner.clone());
                package.member_accounts.sort();
                package.member_accounts.dedup();
            }
            state_transaction.world.musubi_package_members.insert(
                MusubiPackageMemberKeyV1::new(self.package.clone(), owner.clone()),
                MusubiPackageMemberV1 {
                    package: self.package.clone(),
                    account: owner.clone(),
                    role: MusubiPackageRoleV1::Owner,
                    accepted_at_height: execution_height(state_transaction),
                    governance_revision: next,
                },
            );
        }
        package.owners = self.owners;
        package.revisions.governance = next;
        package
            .validate()
            .map_err(|error| invariant(error.reason()))?;
        let owner_count = u8::try_from(package.owners.len())
            .map_err(|_| invariant("Musubi package owner count overflows u8"))?;
        let event = MusubiEvent::PackageRecovered(MusubiPackageRecoveredEventV1 {
            package: self.package.clone(),
            action_digest: self.decision.action_digest,
            owner_count,
            governance_revision: next,
            finalized_height: execution_height(state_transaction),
        });
        state_transaction
            .world
            .musubi_packages
            .insert(self.package, package);
        consume_parliament_decision(self.decision, state_transaction);
        emit_musubi_event(event, state_transaction);
        Ok(())
    }
}

impl Execute for RetargetMusubiAliasV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let action = MusubiParliamentActionV1::RetargetAlias(MusubiRetargetAliasV1 {
            alias: self.alias.clone(),
            target: self.target.clone(),
            expected_revision: self.expected_history_revision,
        });
        verify_parliament_decision(&self.decision, &action, state_transaction)?;
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
        ensure_revision(
            "alias history",
            alias.history_revision,
            self.expected_history_revision,
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
        consume_parliament_decision(self.decision, state_transaction);
        let _ = bump_resolver_index_revision(state_transaction)?;
        emit_musubi_event(event, state_transaction);
        Ok(())
    }
}

impl Execute for SetMusubiArtifactTakedownV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let action = MusubiParliamentActionV1::TakedownArtifact(MusubiTakedownArtifactActionV1 {
            release: self.release.clone(),
            reason: self.reason.clone(),
            expected_artifact_governance_revision: self.expected_artifact_governance_revision,
        });
        verify_parliament_decision(&self.decision, &action, state_transaction)?;
        let mut release = state_transaction
            .world
            .musubi_releases
            .get(&self.release)
            .cloned()
            .ok_or_else(|| release_not_found(&self.release))?;
        ensure_revision(
            "artifact governance",
            release.revisions.artifact_governance,
            self.expected_artifact_governance_revision,
        )?;
        if !matches!(
            release.artifact_governance,
            MusubiArtifactGovernanceStateV1::Available
        ) {
            return Err(invariant("Musubi artifact is already taken down"));
        }
        let next = next_revision(release.revisions.artifact_governance, "artifact governance")?;
        let governance = MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
            action_digest: self.decision.action_digest,
            reason: self.reason,
            enacted_at_height: execution_height(state_transaction),
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
        consume_parliament_decision(self.decision, state_transaction);
        refresh_directory_for_package(&self.release.package, index_revision, state_transaction)?;
        emit_musubi_event(event, state_transaction);
        Ok(())
    }
}

impl Execute for SetMusubiRegistryPolicyV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let action = MusubiParliamentActionV1::SetRegistryPolicy(MusubiSetRegistryPolicyActionV1 {
            policy: self.policy.clone(),
            expected_revision: self.expected_policy_revision,
        });
        verify_parliament_decision(&self.decision, &action, state_transaction)?;
        self.policy
            .validate()
            .map_err(|error| invalid_parameter(error.reason()))?;
        let current_revision = state_transaction
            .world
            .musubi_registry_policy
            .get()
            .revision;
        ensure_revision(
            "registry policy",
            current_revision,
            self.expected_policy_revision,
        )?;
        if self.policy.revision != next_revision(self.expected_policy_revision, "registry policy")?
        {
            return Err(invalid_parameter(
                "Musubi replacement policy revision must be the exact successor",
            ));
        }
        let allowlisted_dataspaces = u16::try_from(self.policy.allowlisted_dataspaces.len())
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
        consume_parliament_decision(self.decision, state_transaction);
        let _ = bump_resolver_index_revision(state_transaction)?;
        emit_musubi_event(event, state_transaction);
        Ok(())
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
    ) -> Result<MusubiReleaseRecordV1, QueryExecutionFail> {
        self.request.release.validate().map_err(query_invalid)?;
        state_ro
            .world()
            .musubi_releases()
            .get(&self.request.release)
            .cloned()
            .ok_or(QueryExecutionFail::NotFound)
    }
}

impl ValidSingularQuery for FindMusubiResolverIndexV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiResolverIndexPageV1, QueryExecutionFail> {
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

impl ValidSingularQuery for FindMusubiVersionsV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiVersionPageV1, QueryExecutionFail> {
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
        Ok(MusubiVersionPageV1 {
            items,
            next_cursor,
            snapshot,
        })
    }
}

impl ValidSingularQuery for FindMusubiMaintainersV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiMaintainerPageV1, QueryExecutionFail> {
        self.request.package.validate().map_err(query_invalid)?;
        let snapshot = query_snapshot(state_ro)?;
        let query_hash = package_page_query_hash(b"maintainers", &self.request);
        let package = state_ro
            .world()
            .musubi_packages()
            .get(&self.request.package)
            .ok_or(QueryExecutionFail::NotFound)?;
        let rows = package
            .member_accounts
            .iter()
            .map(|account| {
                let key =
                    MusubiPackageMemberKeyV1::new(self.request.package.clone(), account.clone());
                let member = state_ro
                    .world()
                    .musubi_package_members()
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| {
                        QueryExecutionFail::Conversion(
                            "Musubi package member directory is inconsistent".to_owned(),
                        )
                    })?;
                Ok((format!("{}|{}", key.package, key.account), member))
            })
            .collect::<Result<Vec<_>, QueryExecutionFail>>()?;
        let (items, next_cursor) = paginate(rows, &self.request.page, query_hash, snapshot)?;
        Ok(MusubiMaintainerPageV1 {
            items,
            next_cursor,
            snapshot,
        })
    }
}

impl ValidSingularQuery for FindMusubiArchiveLocationsV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiArchiveLocationPageV1, QueryExecutionFail> {
        if self.request.archive_id.is_zero() {
            return Err(QueryExecutionFail::Conversion(
                "Musubi archive id must not be zero".to_owned(),
            ));
        }
        let snapshot = query_snapshot(state_ro)?;
        let query_hash = archive_location_query_hash(&self.request);
        let archive = state_ro
            .world()
            .musubi_archives()
            .get(&self.request.archive_id)
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
        Ok(MusubiArchiveLocationPageV1 {
            items,
            next_cursor,
            snapshot,
        })
    }
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

impl ValidSingularQuery for FindMusubiAliasHistoryV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiAliasHistoryPageV1, QueryExecutionFail> {
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
        Ok(MusubiAliasHistoryPageV1 {
            items,
            next_cursor,
            snapshot,
        })
    }
}

impl ValidSingularQuery for FindMusubiOrderedPrefixV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<MusubiOrderedPackagePageV1, QueryExecutionFail> {
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
) -> Result<MusubiAliasHistoryKeyV1, QueryExecutionFail> {
    let revision = if let Some(cursor) = &request.page.cursor {
        let (alias, revision) = cursor
            .last_key
            .rsplit_once(':')
            .ok_or(QueryExecutionFail::Expired)?;
        if alias != request.alias.as_str() || revision.len() != 20 {
            return Err(QueryExecutionFail::Expired);
        }
        revision
            .parse::<u64>()
            .map_err(|_| QueryExecutionFail::Expired)?
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
) -> Result<(MusubiPackageSelectorV1, MusubiNamespaceV1, String), QueryExecutionFail> {
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
        ));
    }
    let namespace = namespace_raw
        .parse::<MusubiNamespaceV1>()
        .map_err(query_invalid)?;
    let start = if let Some(cursor) = &request.page.cursor {
        let selector = cursor
            .last_key
            .parse::<MusubiPackageSelectorV1>()
            .map_err(|_| QueryExecutionFail::Expired)?;
        if selector.namespace != namespace || !selector.name.as_str().starts_with(name_prefix) {
            return Err(QueryExecutionFail::Expired);
        }
        selector
    } else {
        let lower_name = if name_prefix.is_empty() {
            "0".to_owned()
        } else if name_prefix.ends_with('-') {
            if name_prefix.len() == MUSUBI_MAX_PACKAGE_NAME_BYTES_V1 {
                return Err(QueryExecutionFail::Conversion(
                    "Musubi ordered directory package prefix cannot match a package".to_owned(),
                ));
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
) -> Result<(Vec<T>, Option<MusubiFinalizedCursorV1>), QueryExecutionFail> {
    page.validate().map_err(query_invalid)?;
    let cursor_last_key = if let Some(cursor) = &page.cursor {
        if cursor.snapshot != snapshot || cursor.query_hash != query_hash || cursor.caller.is_some()
        {
            return Err(QueryExecutionFail::Expired);
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
        return Err(QueryExecutionFail::Expired);
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
            caller: None,
        })
    } else {
        None
    };
    Ok((items, next_cursor))
}

fn query_invalid(error: iroha_data_model::ParseError) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}

#[derive(Clone, Copy)]
enum PackageCapability {
    Publish,
    Yank,
    Metadata,
    ArchiveLocations,
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
) -> Result<(), Error> {
    let record = world
        .musubi_packages()
        .get(package)
        .ok_or_else(|| package_not_found(package))?;
    if record.owners.binary_search(authority).is_ok() {
        Ok(())
    } else {
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
        Err(invariant(format!(
            "authority '{authority}' lacks the required Musubi package capability for '{package}'"
        )))
    }
}

fn ensure_archive_manager(
    archive: &MusubiArchiveRecordV1,
    authority: &AccountId,
    world: &impl WorldReadOnly,
) -> Result<(), Error> {
    if &archive.registered_by == authority {
        return Ok(());
    }
    let Some(references) = world
        .musubi_archive_reverse_references()
        .get(&archive.archive_id)
    else {
        return Err(invariant("authority cannot manage this Musubi archive"));
    };
    if references.releases.is_empty() {
        return Err(invariant("authority cannot manage this Musubi archive"));
    }
    for release in &references.releases {
        ensure_package_capability(
            &release.package,
            authority,
            PackageCapability::ArchiveLocations,
            world,
        )?;
    }
    Ok(())
}

fn ensure_namespace_owner(
    binding: &MusubiNamespaceBindingV1,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let (owner, generation) = namespace_owner_and_generation(binding, state_transaction)?;
    binding
        .validate_authority_generation(generation)
        .map_err(|error| invalid_parameter(error.reason()))?;
    if owner == *authority {
        Ok(())
    } else {
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

fn ensure_namespace_claim_authority(
    binding: &MusubiNamespaceBindingV1,
    delegation: Option<&MusubiNamespaceDelegationV1>,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let (owner, generation) = namespace_owner_and_generation(binding, state_transaction)?;
    binding
        .validate_authority_generation(generation)
        .map_err(|error| invariant(error.reason()))?;
    if owner == *authority {
        return Ok(());
    }
    let delegation = delegation.ok_or_else(|| {
        invariant(format!(
            "authority '{authority}' neither owns nor has a delegation for Musubi namespace '{}'",
            binding.namespace
        ))
    })?;
    delegation
        .verify(
            binding,
            &owner,
            generation,
            authority,
            execution_height(state_transaction),
        )
        .map_err(|error| invariant(error.reason()))
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
) -> Result<(), Error> {
    match policy.mode {
        MusubiRegistryAdmissionModeV1::Open => Ok(()),
        MusubiRegistryAdmissionModeV1::Closed => Err(invariant(
            "Musubi registry admission is closed for new records",
        )),
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
                Err(invariant(
                    "Musubi registry admission requires an allowlisted dataspace",
                ))
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

fn validate_sorafs_location(
    archive: &MusubiArchiveRecordV1,
    pin_manifest: &iroha_data_model::sorafs::pin_registry::ManifestDigest,
    replication_order: &iroha_data_model::sorafs::pin_registry::ReplicationOrderId,
    provider_attestations: &[MusubiProviderBundleVerificationAttestationV1],
    expires_at_epoch: u64,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Vec<iroha_data_model::sorafs::capacity::ProviderId>, Error> {
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
    providers.dedup();
    if providers.is_empty() || providers.len() > MUSUBI_MAX_LOCATION_PROVIDERS_V1 {
        return Err(invariant(
            "Musubi replication order has an invalid provider completion set",
        ));
    }
    if provider_attestations.len() != providers.len()
        || provider_attestations
            .iter()
            .zip(&providers)
            .any(|(attestation, provider)| attestation.payload.binding.provider_id != *provider)
    {
        return Err(invariant(
            "Musubi provider attestations must exactly cover the sorted finalized completion set",
        ));
    }
    let chain_id = state_transaction.chain_id().clone();
    let genesis_block_hash = genesis_block_hash(state_transaction)?;
    let verification_lock_digest = provider_attestations
        .first()
        .expect("the finalized provider set is non-empty")
        .payload
        .binding
        .verification_lock_digest;
    for attestation in provider_attestations {
        let binding = &attestation.payload.binding;
        let completion = order
            .provider_completion(binding.provider_id)
            .ok_or_else(|| invariant("Musubi provider attestation has no finalized completion"))?;
        let current_owner = state_transaction
            .world
            .provider_owners
            .get(&binding.provider_id)
            .ok_or_else(|| invariant("Musubi provider attestation owner is no longer admitted"))?;
        if binding.chain_id != chain_id
            || binding.genesis_block_hash != genesis_block_hash
            || current_owner != &completion.completed_by
            || current_owner != &completion.completion_authority.provider_owner
            || binding.completed_by != completion.completed_by
            || binding.completion_authority != completion.completion_authority
            || binding.replication_order != *replication_order
            || binding.assignment_revision != completion.assignment_revision
            || binding.completion_epoch != completion.completion_epoch
            || binding.finalized_anchor != completion.finalized_anchor
            || binding.archive_id != archive.archive_id
            || binding.bundle_digest != commitment.bundle_digest
            || binding.descriptor_digest != commitment.descriptor_digest
            || binding.verification_lock_digest != verification_lock_digest
            || binding.semantic_release_manifest_digest
                != archive
                    .staging_receipt
                    .payload
                    .binding
                    .semantic_release_manifest_digest
            || binding.source_tree_digest != commitment.source_tree_digest
        {
            return Err(invariant(
                "Musubi provider attestation does not match the finalized completion or bundle commitment",
            ));
        }
        attestation
            .verify(binding)
            .map_err(|error| invariant(error.reason()))?;
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
        for attestation in &location.provider_attestations {
            let binding = &attestation.payload.binding;
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
    let height = u64::try_from(state_transaction.block_hashes().len())
        .map_err(|_| invariant("Musubi finalized height overflows u64"))?;
    let hash = state_transaction
        .block_hashes()
        .last()
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| invariant("Musubi publication requires a finalized registry snapshot"))?;
    let revision = state_transaction
        .world
        .musubi_resolver_index_revision
        .get()
        .get();
    if snapshot.finalized_height != height
        || snapshot.finalized_block_hash != hash
        || snapshot.index_revision != revision
    {
        return Err(invariant(
            "Musubi publication proof does not use the current finalized registry snapshot",
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

fn add_archive_reverse_reference(
    archive_id: ArchiveId,
    release: MusubiReleaseIdV1,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let mut references = state_transaction
        .world
        .musubi_archive_reverse_references
        .get(&archive_id)
        .cloned()
        .unwrap_or(MusubiArchiveReverseReferencesV1 {
            archive_id,
            releases: Vec::new(),
        });
    references.releases.push(release);
    references.releases.sort();
    references.releases.dedup();
    references
        .validate()
        .map_err(|error| invariant(error.reason()))?;
    state_transaction
        .world
        .musubi_archive_reverse_references
        .insert(archive_id, references);
    Ok(())
}

fn current_location_providers(
    location: &MusubiArchiveLocationV1,
    world: &impl WorldReadOnly,
) -> Option<Vec<iroha_data_model::sorafs::capacity::ProviderId>> {
    if location.state == MusubiArchiveLocationStateV1::Retired {
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
    let providers = location
        .providers
        .iter()
        .zip(&location.provider_attestations)
        .filter_map(|(provider, attestation)| {
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
            let binding = &attestation.payload.binding;
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

/// Prevent a direct SoraFS lifecycle change from removing the last fetchable
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
        let has_remaining = archive.location_ids.iter().any(|location_id| {
            let key = MusubiArchiveLocationKeyV1::new(archive_id, *location_id);
            !excluded.contains(&key)
                && world
                    .musubi_archive_locations()
                    .get(&key)
                    .and_then(|location| current_location_providers(location, world))
                    .is_some_and(|providers| !providers.is_empty())
        });
        if !has_remaining {
            return Err(invariant(
                "Musubi active or yanked releases must retain a valid fetchable archive location",
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
        .copied();
    let location_ids = state_transaction
        .world
        .musubi_archives
        .get(&archive_id)
        .map(|archive| archive.location_ids.clone())
        .ok_or_else(|| archive_not_found(archive_id))?;
    let locations = location_ids
        .iter()
        .map(|location_id| {
            let key = MusubiArchiveLocationKeyV1::new(archive_id, *location_id);
            state_transaction
                .world
                .musubi_archive_locations
                .get(&key)
                .cloned()
                .ok_or_else(|| invariant("Musubi archive location directory is inconsistent"))
        })
        .collect::<Result<Vec<_>, _>>()?;
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
    if previous.is_some_and(|record| {
        record.availability == availability
            && record.healthy_replicas == healthy_replicas
            && record.active_locations == active_locations
    }) {
        return Ok(());
    }
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
    let references = state_transaction
        .world
        .musubi_archive_reverse_references
        .get(&archive_id)
        .map(|record| record.releases.clone())
        .unwrap_or_default();
    #[cfg(feature = "telemetry")]
    {
        if previous
            .is_some_and(|record| record.availability == MusubiStorageAvailabilityV1::Selectable)
            != (projection.availability == MusubiStorageAvailabilityV1::Selectable)
        {
            let release_count = u64::try_from(references.len())
                .map_err(|_| invariant("Musubi archive reverse-reference count overflows u64"))?;
            if projection.availability == MusubiStorageAvailabilityV1::Selectable {
                state_transaction
                    .telemetry
                    .adjust_musubi_replication_shortfall_releases(0, release_count);
            } else {
                state_transaction
                    .telemetry
                    .adjust_musubi_replication_shortfall_releases(release_count, 0);
            }
        }
    }
    let mut packages = BTreeSet::new();
    for release in references {
        if let Some(mut row) = state_transaction
            .world
            .musubi_resolver_index
            .get(&release)
            .cloned()
        {
            row.selection.storage = projection;
            row.index_revision = index_revision;
            packages.insert(release.package.clone());
            state_transaction
                .world
                .musubi_resolver_index
                .insert(release, row);
        }
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
    let metadata_revision = package_record.revisions.metadata;
    let latest_selectable = state_transaction
        .world
        .musubi_resolver_index
        .range(package_release_start(package)..)
        .take_while(|(release, _)| release.package == *package)
        .filter(|(_, row)| row.selection.fresh_selectable())
        .map(|(release, _)| release.version.clone())
        .max();
    let selector = MusubiPackageSelectorV1 {
        namespace: package_record.claimed_namespace,
        name: package.name.clone(),
    };
    state_transaction.world.musubi_public_directory.insert(
        selector.clone(),
        MusubiOrderedPackageEntryV1 {
            selector,
            package: package.clone(),
            latest_selectable,
            metadata_revision,
            index_revision,
        },
    );
    Ok(())
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
) -> Result<MusubiReleaseIdV1, QueryExecutionFail> {
    if let Some(cursor) = &page.cursor {
        let version = cursor
            .last_key
            .parse::<MusubiVersionV1>()
            .map_err(|_| QueryExecutionFail::Expired)?;
        Ok(MusubiReleaseIdV1::new(package.clone(), version))
    } else {
        Ok(package_release_start(package))
    }
}

fn verify_parliament_decision(
    decision: &MusubiGovernanceDecisionV1,
    action: &MusubiParliamentActionV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    decision
        .validate()
        .map_err(|error| invalid_parameter(error.reason()))?;
    action
        .validate()
        .map_err(|error| invalid_parameter(error.reason()))?;
    if decision.action_digest != action.action_digest() {
        return Err(invariant(
            "Musubi Parliament decision does not bind the exact requested action",
        ));
    }
    if state_transaction
        .world
        .musubi_governance_decisions
        .get(&decision.decision_id)
        .is_some()
    {
        return Err(invariant("Musubi Parliament decision was already consumed"));
    }
    let proposal = state_transaction
        .world
        .governance_proposals
        .get(&decision.decision_id)
        .ok_or_else(|| invariant("Musubi Parliament decision has no governance proposal"))?;
    if proposal.status != GovernanceProposalStatus::Enacted
        || proposal.enacted_at_height != Some(decision.enacted_at_height)
    {
        return Err(invariant(
            "Musubi Parliament proposal is not enacted at the claimed height",
        ));
    }
    match &proposal.kind {
        ProposalKind::MusubiRegistryGovernance(enacted) if enacted == action => {}
        _ => {
            return Err(invariant(
                "Musubi Parliament proposal payload does not match the requested action",
            ));
        }
    }
    let minimum = decision
        .enacted_at_height
        .checked_add(state_transaction.gov.min_enactment_delay)
        .ok_or_else(|| invariant("Musubi Parliament delay overflows block height"))?;
    if decision.execute_after_height < minimum
        || execution_height(state_transaction) < decision.execute_after_height
    {
        return Err(invariant(
            "Musubi Parliament decision has not satisfied the enactment delay",
        ));
    }
    Ok(())
}

fn consume_parliament_decision(
    decision: MusubiGovernanceDecisionV1,
    state_transaction: &mut StateTransaction<'_, '_>,
) {
    state_transaction
        .world
        .musubi_governance_decisions
        .insert(decision.decision_id, decision);
}

fn bump_resolver_index_revision(
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<u64, Error> {
    let current = *state_transaction.world.musubi_resolver_index_revision.get();
    let next = current
        .checked_next()
        .ok_or_else(|| invariant("Musubi resolver-index revision overflow"))?;
    *state_transaction
        .world
        .musubi_resolver_index_revision
        .get_mut() = next;
    Ok(next.get())
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
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

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
            provider_attestations: Vec::new(),
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

    fn snapshot(revision: u64) -> MusubiRegistrySnapshotV1 {
        MusubiRegistrySnapshotV1 {
            finalized_height: 7,
            finalized_block_hash: [7; 32],
            index_revision: revision,
        }
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
    fn pagination_rejects_stale_index_revision_and_last_key() {
        let hash = query_hash(b"test", b"request");
        let cursor = MusubiFinalizedCursorV1 {
            snapshot: snapshot(1),
            query_hash: hash,
            last_key: "missing".to_owned(),
            caller: None,
        };
        let request = MusubiPageRequestV1 {
            limit: 1,
            cursor: Some(cursor),
        };
        assert!(matches!(
            paginate(vec![("a".to_owned(), 1_u8)], &request, hash, snapshot(1)),
            Err(QueryExecutionFail::Expired)
        ));

        let cursor = MusubiFinalizedCursorV1 {
            snapshot: snapshot(1),
            query_hash: hash,
            last_key: "a".to_owned(),
            caller: None,
        };
        let request = MusubiPageRequestV1 {
            limit: 1,
            cursor: Some(cursor),
        };
        assert!(matches!(
            paginate(vec![("a".to_owned(), 1_u8)], &request, hash, snapshot(2)),
            Err(QueryExecutionFail::Expired)
        ));
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
    fn idempotent_and_rejected_namespace_bindings_emit_no_events() {
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

        RegisterMusubiNamespaceBindingV1::new(binding.clone(), 1)
            .execute(&account(1), &mut transaction)
            .expect("identical namespace registration is idempotent");
        assert!(transaction.world.take_external_events().is_empty());

        let conflicting = MusubiNamespaceBindingV1 {
            home_dataspace: iroha_data_model::nexus::DataSpaceId::new(8),
            ..binding
        };
        RegisterMusubiNamespaceBindingV1::new(conflicting, 1)
            .execute(&account(1), &mut transaction)
            .expect_err("conflicting immutable namespace binding is rejected");
        assert!(transaction.world.take_external_events().is_empty());
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
