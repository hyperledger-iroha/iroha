//! Bounded Musubi V1 registry and storage lifecycle events.
use iroha_data_model_derive::model;
pub use self::model::*;
use super::*;
#[model]
mod model {
    use super::*;
    /// Closed family of finalized Musubi registry transitions.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        iroha_data_model_derive::EventSet,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum MusubiEvent {
        /// A canonical namespace was immutably bound.
        NamespaceBound(crate::musubi::MusubiNamespaceBindingV1),
        /// First publication claimed a package.
        PackageClaimed(MusubiPackageClaimedEventV1),
        /// An immutable release was published.
        ReleasePublished(MusubiReleasePublishedEventV1),
        /// A release was yanked or unyanked.
        ReleaseYankChanged(MusubiReleaseYankEventV1),
        /// Mutable package metadata changed.
        PackageMetadataChanged(crate::musubi::MusubiPackageMetadataRecordV1),
        /// A package member invitation was created.
        MaintainerInvited(crate::musubi::MusubiMaintainerInvitationV1),
        /// An invited package member accepted.
        MaintainerAccepted(crate::musubi::MusubiPackageMemberV1),
        /// A height-expired pending invitation was deterministically closed.
        MaintainerInvitationExpired(MusubiMaintainerInvitationLifecycleEventV1),
        /// A package owner revoked a pending invitation.
        MaintainerInvitationRevoked(MusubiMaintainerInvitationLifecycleEventV1),
        /// An accepted package member's role changed.
        MaintainerRoleChanged(crate::musubi::MusubiPackageMemberV1),
        /// An accepted package member was removed.
        MaintainerRemoved(MusubiPackageMemberRemovedEventV1),
        /// Parliament recovered package ownership.
        PackageRecovered(MusubiPackageRecoveredEventV1),
        /// A paid permanent alias was registered.
        AliasRegistered(crate::musubi::MusubiAliasHistoryEntryV1),
        /// Parliament retargeted a permanent alias while retaining history.
        AliasRetargeted(crate::musubi::MusubiAliasHistoryEntryV1),
        /// A canonical archive commitment was registered.
        ArchiveRegistered(MusubiArchiveRegisteredEventV1),
        /// A provider's complete bundle-verification attestation was immutably registered.
        ProviderBundleAttestationRegistered(MusubiProviderBundleAttestationRegisteredEventV1),
        /// An archive location was added, renewed, retired, or refreshed.
        ArchiveLocationChanged(MusubiArchiveLocationEventV1),
        /// Aggregate archive availability changed.
        ArchiveAvailabilityChanged(crate::musubi::MusubiArchiveAvailabilityV1),
        /// Parliament made a release artifact unavailable.
        ArtifactTakenDown(MusubiArtifactTakedownEventV1),
        /// Parliament replaced the chain-wide registry policy.
        RegistryPolicyChanged(MusubiRegistryPolicyEventV1),
    }
    /// Compact first-package-claim event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiPackageClaimedEventV1 {
        /// Stable structural package identity.
        pub package: crate::musubi::MusubiPackageIdV1,
        /// Canonical namespace used for the immutable claim.
        pub namespace: crate::musubi::MusubiNamespaceV1,
        /// First package owner.
        pub claimed_by: AccountId,
        /// Initial governance revision.
        pub governance_revision: u64,
        /// Finalized claim height.
        pub finalized_height: u64,
    }
    /// Compact immutable-release publication event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiReleasePublishedEventV1 {
        /// Exact package release.
        pub release: crate::musubi::MusubiReleaseIdV1,
        /// Full immutable release digest.
        pub release_digest: crate::musubi::MusubiReleaseDigestV1,
        /// Canonical source archive.
        pub archive_id: crate::musubi::ArchiveId,
        /// Publishing account.
        pub published_by: AccountId,
        /// Finalized publication height.
        pub finalized_height: u64,
    }
    /// Compact yank/unyank event with its immutable archive binding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiReleaseYankEventV1 {
        /// Resulting reversible yank record.
        pub yank: crate::musubi::MusubiReleaseYankV1,
        /// Immutable release archive, enabling exact archive-aware filtering.
        pub archive_id: crate::musubi::ArchiveId,
    }
    /// Compact accepted-member removal event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiPackageMemberRemovedEventV1 {
        /// Governed package.
        pub package: crate::musubi::MusubiPackageIdV1,
        /// Removed account.
        pub account: AccountId,
        /// Role held immediately before removal.
        pub previous_role: crate::musubi::MusubiPackageRoleV1,
        /// Resulting package-governance revision.
        pub governance_revision: u64,
        /// Finalized removal height.
        pub finalized_height: u64,
    }
    /// Compact terminal transition for a pending package-member invitation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiMaintainerInvitationLifecycleEventV1 {
        /// Governed package.
        pub package: crate::musubi::MusubiPackageIdV1,
        /// Closed invitation identity.
        pub invite_id: crate::musubi::MusubiInviteIdV1,
        /// Account named by the invitation.
        pub invited_account: AccountId,
        /// Resulting package-governance revision.
        pub governance_revision: u64,
        /// Finalized transition height.
        pub finalized_height: u64,
    }
    /// Compact Parliament package-recovery event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiPackageRecoveredEventV1 {
        /// Recovered package.
        pub package: crate::musubi::MusubiPackageIdV1,
        /// Domain-separated digest of the enacted recovery action.
        pub action_digest: crate::musubi::MusubiGovernanceActionDigestV1,
        /// Resulting owner count; exact owners remain in authoritative package state.
        pub owner_count: u8,
        /// Resulting package-governance revision.
        pub governance_revision: u64,
        /// Finalized recovery height.
        pub finalized_height: u64,
    }
    /// Compact archive-registration event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiArchiveRegisteredEventV1 {
        /// Domain-separated archive identity.
        pub archive_id: crate::musubi::ArchiveId,
        /// Account that registered the authenticated ingress commitment.
        pub registered_by: AccountId,
        /// Initial location-set revision.
        pub location_revision: u64,
        /// Finalized registration height.
        pub finalized_height: u64,
    }
    /// Compact provider bundle-attestation registration event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiProviderBundleAttestationRegisteredEventV1 {
        /// Exact archive/order/provider identity of the registered attestation.
        pub key: crate::musubi::MusubiProviderBundleAttestationKeyV1,
        /// Domain-separated digest of the complete bounded attestation.
        pub attestation_digest: crate::musubi::MusubiProviderBundleAttestationDigestV1,
        /// Archive manager that registered the immutable attestation.
        pub registered_by: AccountId,
        /// Finalized registration height.
        pub finalized_height: u64,
    }
    /// Closed archive-location transition kind.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum MusubiArchiveLocationTransitionV1 {
        /// A new location identity was bound.
        Added,
        /// An existing non-retired location was renewed with current evidence.
        Renewed,
        /// A package owner retired the location.
        Retired,
        /// Underlying `SoraFS` evidence changed its health state.
        EvidenceRefreshed,
    }
    /// Compact archive-location lifecycle event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiArchiveLocationEventV1 {
        /// Exact archive/location identity.
        pub location: crate::musubi::MusubiArchiveLocationKeyV1,
        /// Exact current pin-manifest evidence.
        pub pin_manifest: crate::sorafs::pin_registry::ManifestDigest,
        /// Exact current replication-order evidence.
        pub replication_order: crate::sorafs::pin_registry::ReplicationOrderId,
        /// Digest of the exact sorted provider-attestation set used by the transition.
        pub provider_attestation_set_digest:
            crate::musubi::MusubiProviderBundleAttestationSetDigestV1,
        /// Number of distinct providers committed by the attestation set.
        pub provider_count: u8,
        /// Transition classification.
        pub transition: MusubiArchiveLocationTransitionV1,
        /// Resulting location health state.
        pub state: crate::musubi::MusubiArchiveLocationStateV1,
        /// Resulting compare-and-set revision.
        pub revision: u64,
        /// Finalized transition height.
        pub finalized_height: u64,
    }
    /// Compact Parliament artifact-takedown event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiArtifactTakedownEventV1 {
        /// Exact release whose artifact became unavailable.
        pub release: crate::musubi::MusubiReleaseIdV1,
        /// Immutable release archive.
        pub archive_id: crate::musubi::ArchiveId,
        /// Enacted Parliament action digest.
        pub action_digest: crate::musubi::MusubiGovernanceActionDigestV1,
        /// Resulting artifact-governance revision.
        pub governance_revision: u64,
        /// Finalized height where the delayed action was applied.
        pub finalized_height: u64,
    }
    /// Compact registry-policy replacement event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MusubiRegistryPolicyEventV1 {
        /// Resulting policy revision.
        pub revision: u64,
        /// Resulting admission mode.
        pub mode: crate::musubi::MusubiRegistryAdmissionModeV1,
        /// Prospective alias-pricing revision.
        pub alias_pricing_revision: u64,
        /// Bounded allowlist cardinality without identity-valued labels.
        pub allowlisted_dataspaces: u16,
        /// Enacted Parliament action digest.
        pub action_digest: crate::musubi::MusubiGovernanceActionDigestV1,
        /// Finalized enactment height.
        pub finalized_height: u64,
    }
}
impl MusubiEvent {
    /// Return the structural package affected by this event, when any.
    #[must_use]
    pub fn package(&self) -> Option<&crate::musubi::MusubiPackageIdV1> {
        match self {
            Self::PackageClaimed(event) => Some(&event.package),
            Self::ReleasePublished(event) => Some(&event.release.package),
            Self::ReleaseYankChanged(event) => Some(&event.yank.release.package),
            Self::PackageMetadataChanged(event) => Some(&event.package),
            Self::MaintainerInvited(event) => Some(&event.package),
            Self::MaintainerAccepted(event) | Self::MaintainerRoleChanged(event) => {
                Some(&event.package)
            }
            Self::MaintainerInvitationExpired(event) | Self::MaintainerInvitationRevoked(event) => {
                Some(&event.package)
            }
            Self::MaintainerRemoved(event) => Some(&event.package),
            Self::PackageRecovered(event) => Some(&event.package),
            Self::AliasRegistered(event) | Self::AliasRetargeted(event) => Some(&event.target),
            Self::ArtifactTakenDown(event) => Some(&event.release.package),
            Self::NamespaceBound(_)
            | Self::ArchiveRegistered(_)
            | Self::ProviderBundleAttestationRegistered(_)
            | Self::ArchiveLocationChanged(_)
            | Self::ArchiveAvailabilityChanged(_)
            | Self::RegistryPolicyChanged(_) => None,
        }
    }
    /// Return the archive affected by this event, when any.
    #[must_use]
    pub const fn archive(&self) -> Option<crate::musubi::ArchiveId> {
        match self {
            Self::ReleasePublished(event) => Some(event.archive_id),
            Self::ReleaseYankChanged(event) => Some(event.archive_id),
            Self::ArchiveRegistered(event) => Some(event.archive_id),
            Self::ProviderBundleAttestationRegistered(event) => Some(event.key.archive_id),
            Self::ArchiveLocationChanged(event) => Some(event.location.archive_id),
            Self::ArchiveAvailabilityChanged(event) => Some(event.archive_id),
            Self::ArtifactTakenDown(event) => Some(event.archive_id),
            _ => None,
        }
    }
    /// Return the permanent alias affected by this event, when any.
    #[must_use]
    pub fn alias(&self) -> Option<&crate::musubi::MusubiAliasNameV1> {
        match self {
            Self::AliasRegistered(event) | Self::AliasRetargeted(event) => Some(&event.alias),
            _ => None,
        }
    }
}
/// Common Musubi event exports.
pub mod prelude {
    pub use super::{
        MusubiArchiveLocationEventV1, MusubiArchiveLocationTransitionV1,
        MusubiArchiveRegisteredEventV1, MusubiArtifactTakedownEventV1, MusubiEvent, MusubiEventSet,
        MusubiMaintainerInvitationLifecycleEventV1, MusubiPackageClaimedEventV1,
        MusubiPackageMemberRemovedEventV1, MusubiPackageRecoveredEventV1,
        MusubiProviderBundleAttestationRegisteredEventV1, MusubiRegistryPolicyEventV1,
        MusubiReleasePublishedEventV1, MusubiReleaseYankEventV1,
    };
}
#[cfg(test)]
mod tests {
    use norito::codec::{DecodeAll as _, Encode as _};
    use super::*;
    #[test]
    fn provider_attestation_registration_event_is_compact_and_archive_routable() {
        let archive_id = crate::musubi::ArchiveId::new([0x31; 32]);
        let event = MusubiEvent::ProviderBundleAttestationRegistered(
            MusubiProviderBundleAttestationRegisteredEventV1 {
                key: crate::musubi::MusubiProviderBundleAttestationKeyV1 {
                    archive_id,
                    replication_order: crate::sorafs::pin_registry::ReplicationOrderId::new(
                        [0x32; 32],
                    ),
                    provider_id: crate::sorafs::capacity::ProviderId::new([0x33; 32]),
                },
                attestation_digest: crate::musubi::MusubiProviderBundleAttestationDigestV1::new(
                    [0x34; 32],
                ),
                registered_by: AccountId::new(
                    "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                        .parse()
                        .expect("public key"),
                ),
                finalized_height: 7,
            },
        );
        assert_eq!(event.archive(), Some(archive_id));
        assert_eq!(event.package(), None);
        assert_eq!(event.alias(), None);
    }
    #[test]
    fn archive_location_event_roundtrip_carries_only_compact_provider_commitment() {
        let event = MusubiArchiveLocationEventV1 {
            location: crate::musubi::MusubiArchiveLocationKeyV1::new(
                crate::musubi::ArchiveId::new([0x41; 32]),
                crate::musubi::MusubiArchiveLocationIdV1::new([0x42; 32]),
            ),
            pin_manifest: crate::sorafs::pin_registry::ManifestDigest::new([0x43; 32]),
            replication_order: crate::sorafs::pin_registry::ReplicationOrderId::new([0x44; 32]),
            provider_attestation_set_digest:
                crate::musubi::MusubiProviderBundleAttestationSetDigestV1::new([0x45; 32]),
            provider_count: 3,
            transition: MusubiArchiveLocationTransitionV1::Added,
            state: crate::musubi::MusubiArchiveLocationStateV1::Healthy,
            revision: 2,
            finalized_height: 7,
        };
        let decoded = MusubiArchiveLocationEventV1::decode_all(&mut event.encode().as_slice())
            .expect("compact location event Norito roundtrip");
        assert_eq!(decoded, event);
        assert_eq!(decoded.provider_count, 3);
    }
}
