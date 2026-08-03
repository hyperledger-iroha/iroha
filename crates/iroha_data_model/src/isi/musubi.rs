//! First-release Musubi registry instructions.
//!
//! These instructions intentionally have no pre-release compatibility aliases.
//! Every mutable operation carries an explicit compare-and-set or policy
//! revision, and Parliament recovery carries an exact action-bound decision.

use super::*;
use crate::musubi::{
    ArchiveId, MusubiAliasNameV1, MusubiArchiveCommitmentV1, MusubiArchiveLocationIdV1,
    MusubiGovernanceDecisionV1, MusubiNamespaceBindingV1, MusubiNamespaceDelegationV1,
    MusubiNamespaceV1, MusubiPackageIdV1, MusubiPackageRoleV1,
    MusubiProviderBundleVerificationAttestationV1, MusubiPublicationV1, MusubiReasonV1,
    MusubiRegistryPolicyV1, MusubiReleaseDigestV1, MusubiReleaseIdV1, MusubiReleaseMetadataV1,
    MusubiSeedIngressReceiptV1,
};
use crate::sorafs::pin_registry::{ManifestDigest, ReplicationOrderId};

isi! {
    /// Register one immutable namespace-to-home-dataspace binding.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct RegisterMusubiNamespaceBindingV1 {
        /// Immutable namespace binding.
        pub binding: MusubiNamespaceBindingV1,
        /// Exact registry-policy revision observed by the submitter.
        pub expected_policy_revision: u64,
    }
}

impl RegisterMusubiNamespaceBindingV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.namespace_binding.register";

    /// Construct an immutable namespace-binding registration.
    #[must_use]
    pub const fn new(binding: MusubiNamespaceBindingV1, expected_policy_revision: u64) -> Self {
        Self {
            binding,
            expected_policy_revision,
        }
    }
}

impl crate::seal::Instruction for RegisterMusubiNamespaceBindingV1 {}

isi! {
    /// Register an immutable source archive commitment.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct RegisterMusubiArchiveV1 {
        /// Complete commitment whose domain-separated hash is the archive id.
        pub commitment: MusubiArchiveCommitmentV1,
        /// Signed, unexpired receipt for the exact CAR accepted by authenticated seed ingress.
        pub staging_receipt: MusubiSeedIngressReceiptV1,
        /// Exact registry-policy revision observed by the submitter.
        pub expected_policy_revision: u64,
    }
}

impl RegisterMusubiArchiveV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.archive.register";

    /// Construct an archive registration.
    #[must_use]
    pub const fn new(
        commitment: MusubiArchiveCommitmentV1,
        staging_receipt: MusubiSeedIngressReceiptV1,
        expected_policy_revision: u64,
    ) -> Self {
        Self {
            commitment,
            staging_receipt,
            expected_policy_revision,
        }
    }
}

impl crate::seal::Instruction for RegisterMusubiArchiveV1 {}

isi! {
    /// Add or renew one SoraFS location for a registered archive.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct AddMusubiArchiveLocationV1 {
        /// Registered archive identity.
        pub archive_id: ArchiveId,
        /// Stable location identity.
        pub location_id: MusubiArchiveLocationIdV1,
        /// Registry-grade SoraFS pin manifest.
        pub pin_manifest: ManifestDigest,
        /// Replication order whose finalized completions prove availability.
        pub replication_order: ReplicationOrderId,
        /// Sorted, provider-distinct parsed-bundle attestations for finalized completions.
        pub provider_attestations: Vec<MusubiProviderBundleVerificationAttestationV1>,
        /// Earliest epoch at which renewal should occur.
        pub renew_after_epoch: u64,
        /// Epoch after which the location is invalid.
        pub expires_at_epoch: u64,
        /// Compare-and-set archive location revision.
        pub expected_location_revision: u64,
    }
}

impl AddMusubiArchiveLocationV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.archive_location.add";
}

impl crate::seal::Instruction for AddMusubiArchiveLocationV1 {}

isi! {
    /// Retire one archive location without changing archive or release identity.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct RetireMusubiArchiveLocationV1 {
        /// Registered archive identity.
        pub archive_id: ArchiveId,
        /// Exact location to retire.
        pub location_id: MusubiArchiveLocationIdV1,
        /// Compare-and-set archive location revision.
        pub expected_location_revision: u64,
        /// Bounded public retirement reason.
        pub reason: MusubiReasonV1,
    }
}

impl RetireMusubiArchiveLocationV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.archive_location.retire";
}

impl crate::seal::Instruction for RetireMusubiArchiveLocationV1 {}

isi! {
    /// Claim an absent package if authorized and publish one immutable release.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct PublishMusubiReleaseV1 {
        /// Exact canonical namespace whose immutable binding authorizes the package claim.
        pub namespace: MusubiNamespaceV1,
        /// Immutable release plus bounded exact publication proof.
        pub publication: MusubiPublicationV1,
        /// Optional generation-bound namespace delegation for the first claim.
        pub namespace_delegation: Option<MusubiNamespaceDelegationV1>,
        /// Exact registry-policy revision observed by the publisher.
        pub expected_policy_revision: u64,
        /// Existing-package governance revision; absent only for the first claim.
        pub expected_governance_revision: Option<u64>,
    }
}

impl PublishMusubiReleaseV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.release.publish";

    /// Construct a publication request.
    #[must_use]
    pub const fn new(
        namespace: MusubiNamespaceV1,
        publication: MusubiPublicationV1,
        namespace_delegation: Option<MusubiNamespaceDelegationV1>,
        expected_policy_revision: u64,
        expected_governance_revision: Option<u64>,
    ) -> Self {
        Self {
            namespace,
            publication,
            namespace_delegation,
            expected_policy_revision,
            expected_governance_revision,
        }
    }
}

impl crate::seal::Instruction for PublishMusubiReleaseV1 {}

isi! {
    /// Yank or unyank an immutable release.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct SetMusubiReleaseYankV1 {
        /// Exact immutable release.
        pub release: MusubiReleaseIdV1,
        /// `true` yanks; `false` unyanks.
        pub yanked: bool,
        /// Bounded public transition reason.
        pub reason: MusubiReasonV1,
        /// Compare-and-set yank revision.
        pub expected_yank_revision: u64,
    }
}

impl SetMusubiReleaseYankV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.release_yank.set";

    /// Construct a reversible yank transition.
    #[must_use]
    pub const fn new(
        release: MusubiReleaseIdV1,
        yanked: bool,
        reason: MusubiReasonV1,
        expected_yank_revision: u64,
    ) -> Self {
        Self {
            release,
            yanked,
            reason,
            expected_yank_revision,
        }
    }
}

impl crate::seal::Instruction for SetMusubiReleaseYankV1 {}

isi! {
    /// Replace the mutable package metadata projection.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct SetMusubiPackageMetadataV1 {
        /// Stable package identity.
        pub package: MusubiPackageIdV1,
        /// Complete replacement metadata.
        pub metadata: MusubiReleaseMetadataV1,
        /// Compare-and-set metadata revision.
        pub expected_metadata_revision: u64,
    }
}

impl SetMusubiPackageMetadataV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.package_metadata.set";
}

impl crate::seal::Instruction for SetMusubiPackageMetadataV1 {}

isi! {
    /// Invite an account to an owner or maintainer role.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct InviteMusubiPackageMaintainerV1 {
        /// Stable package identity.
        pub package: MusubiPackageIdV1,
        /// Stable invitation identity.
        pub invite_id: crate::musubi::MusubiInviteIdV1,
        /// Account that alone may accept.
        pub invited_account: AccountId,
        /// Offered role.
        pub role: MusubiPackageRoleV1,
        /// Last height at which the invite may be accepted.
        pub expires_at_height: u64,
        /// Compare-and-set governance revision.
        pub expected_governance_revision: u64,
    }
}

impl InviteMusubiPackageMaintainerV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.package_member.invite";
}

impl crate::seal::Instruction for InviteMusubiPackageMaintainerV1 {}

isi! {
    /// Accept a pending package role invitation as its invited account.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct AcceptMusubiPackageMaintainerV1 {
        /// Stable package identity.
        pub package: MusubiPackageIdV1,
        /// Exact invitation identity.
        pub invite_id: crate::musubi::MusubiInviteIdV1,
        /// Compare-and-set governance revision.
        pub expected_governance_revision: u64,
    }
}

impl AcceptMusubiPackageMaintainerV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.package_member.accept";
}

impl crate::seal::Instruction for AcceptMusubiPackageMaintainerV1 {}

isi! {
    /// Revoke a pending package role invitation as a current package owner.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct RevokeMusubiPackageMaintainerInvitationV1 {
        /// Stable package identity.
        pub package: MusubiPackageIdV1,
        /// Exact pending invitation identity.
        pub invite_id: crate::musubi::MusubiInviteIdV1,
        /// Compare-and-set governance revision.
        pub expected_governance_revision: u64,
    }
}

impl RevokeMusubiPackageMaintainerInvitationV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.package_member.invitation.revoke";
}

impl crate::seal::Instruction for RevokeMusubiPackageMaintainerInvitationV1 {}

isi! {
    /// Change an accepted package member's role.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct SetMusubiPackageMaintainerRoleV1 {
        /// Stable package identity.
        pub package: MusubiPackageIdV1,
        /// Accepted member account.
        pub account: AccountId,
        /// Complete replacement role.
        pub role: MusubiPackageRoleV1,
        /// Compare-and-set governance revision.
        pub expected_governance_revision: u64,
    }
}

impl SetMusubiPackageMaintainerRoleV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.package_member.set_role";
}

impl crate::seal::Instruction for SetMusubiPackageMaintainerRoleV1 {}

isi! {
    /// Remove an accepted package member while preserving the last owner.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct RemoveMusubiPackageMaintainerV1 {
        /// Stable package identity.
        pub package: MusubiPackageIdV1,
        /// Accepted member account to remove.
        pub account: AccountId,
        /// Compare-and-set governance revision.
        pub expected_governance_revision: u64,
    }
}

impl RemoveMusubiPackageMaintainerV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.package_member.remove";
}

impl crate::seal::Instruction for RemoveMusubiPackageMaintainerV1 {}

isi! {
    /// Register a paid permanent global package alias.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct RegisterMusubiAliasV1 {
        /// Permanent lowercase ASCII kebab alias.
        pub alias: MusubiAliasNameV1,
        /// Package target normalized before submission.
        pub target: MusubiPackageIdV1,
        /// Exact prospective pricing-policy revision.
        pub expected_pricing_revision: u64,
    }
}

impl RegisterMusubiAliasV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.alias.register";

    /// Construct a permanent alias registration.
    #[must_use]
    pub const fn new(
        alias: MusubiAliasNameV1,
        target: MusubiPackageIdV1,
        expected_pricing_revision: u64,
    ) -> Self {
        Self {
            alias,
            target,
            expected_pricing_revision,
        }
    }
}

impl crate::seal::Instruction for RegisterMusubiAliasV1 {}

isi! {
    /// Apply an enacted Parliament package-owner recovery.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct RecoverMusubiPackageV1 {
        /// Enacted, action-digest-bound decision.
        pub decision: MusubiGovernanceDecisionV1,
        /// Stable package identity.
        pub package: MusubiPackageIdV1,
        /// Sorted replacement owner set.
        pub owners: Vec<AccountId>,
        /// Compare-and-set governance revision.
        pub expected_governance_revision: u64,
    }
}

impl RecoverMusubiPackageV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.parliament.package_recover";
}

impl crate::seal::Instruction for RecoverMusubiPackageV1 {}

isi! {
    /// Apply an enacted Parliament retarget of a permanent alias.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct RetargetMusubiAliasV1 {
        /// Enacted, action-digest-bound decision.
        pub decision: MusubiGovernanceDecisionV1,
        /// Permanent alias.
        pub alias: MusubiAliasNameV1,
        /// New structural target.
        pub target: MusubiPackageIdV1,
        /// Compare-and-set alias-history revision.
        pub expected_history_revision: u64,
    }
}

impl RetargetMusubiAliasV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.parliament.alias_retarget";
}

impl crate::seal::Instruction for RetargetMusubiAliasV1 {}

isi! {
    /// Apply an enacted Parliament artifact takedown.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct SetMusubiArtifactTakedownV1 {
        /// Enacted, action-digest-bound decision.
        pub decision: MusubiGovernanceDecisionV1,
        /// Exact immutable release.
        pub release: MusubiReleaseIdV1,
        /// Bounded public takedown reason.
        pub reason: MusubiReasonV1,
        /// Compare-and-set artifact-governance revision.
        pub expected_artifact_governance_revision: u64,
    }
}

impl SetMusubiArtifactTakedownV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.parliament.artifact_takedown";
}

impl crate::seal::Instruction for SetMusubiArtifactTakedownV1 {}

isi! {
    /// Replace the prospective registry admission and alias pricing policy.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct SetMusubiRegistryPolicyV1 {
        /// Enacted, action-digest-bound decision.
        pub decision: MusubiGovernanceDecisionV1,
        /// Complete replacement policy.
        pub policy: MusubiRegistryPolicyV1,
        /// Compare-and-set current policy revision.
        pub expected_policy_revision: u64,
    }
}

impl SetMusubiRegistryPolicyV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.parliament.registry_policy.set";
}

impl crate::seal::Instruction for SetMusubiRegistryPolicyV1 {}

isi! {
    /// Assert an exact immutable release digest.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    pub struct AssertMusubiReleaseDigestV1 {
        /// Exact release identity.
        pub release: MusubiReleaseIdV1,
        /// Required immutable digest.
        pub expected_digest: MusubiReleaseDigestV1,
    }
}

impl AssertMusubiReleaseDigestV1 {
    /// First-release stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.musubi.v1.release_digest.assert";

    /// Construct an exact release digest assertion.
    #[must_use]
    pub const fn new(release: MusubiReleaseIdV1, expected_digest: MusubiReleaseDigestV1) -> Self {
        Self {
            release,
            expected_digest,
        }
    }
}

impl crate::seal::Instruction for AssertMusubiReleaseDigestV1 {}

fn musubi_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_decode_musubi_instruction {
    ($type:ident { $($field:ident: $field_type:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $type {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = musubi_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_type>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}

impl_decode_musubi_instruction!(RegisterMusubiNamespaceBindingV1 {
    binding: MusubiNamespaceBindingV1,
    expected_policy_revision: u64,
});
impl_decode_musubi_instruction!(RegisterMusubiArchiveV1 {
    commitment: MusubiArchiveCommitmentV1,
    staging_receipt: MusubiSeedIngressReceiptV1,
    expected_policy_revision: u64,
});
impl_decode_musubi_instruction!(AddMusubiArchiveLocationV1 {
    archive_id: ArchiveId,
    location_id: MusubiArchiveLocationIdV1,
    pin_manifest: ManifestDigest,
    replication_order: ReplicationOrderId,
    provider_attestations: Vec<MusubiProviderBundleVerificationAttestationV1>,
    renew_after_epoch: u64,
    expires_at_epoch: u64,
    expected_location_revision: u64,
});
impl_decode_musubi_instruction!(RetireMusubiArchiveLocationV1 {
    archive_id: ArchiveId,
    location_id: MusubiArchiveLocationIdV1,
    expected_location_revision: u64,
    reason: MusubiReasonV1,
});
impl_decode_musubi_instruction!(PublishMusubiReleaseV1 {
    namespace: MusubiNamespaceV1,
    publication: MusubiPublicationV1,
    namespace_delegation: Option<MusubiNamespaceDelegationV1>,
    expected_policy_revision: u64,
    expected_governance_revision: Option<u64>,
});
impl_decode_musubi_instruction!(SetMusubiReleaseYankV1 {
    release: MusubiReleaseIdV1,
    yanked: bool,
    reason: MusubiReasonV1,
    expected_yank_revision: u64,
});
impl_decode_musubi_instruction!(SetMusubiPackageMetadataV1 {
    package: MusubiPackageIdV1,
    metadata: MusubiReleaseMetadataV1,
    expected_metadata_revision: u64,
});
impl_decode_musubi_instruction!(InviteMusubiPackageMaintainerV1 {
    package: MusubiPackageIdV1,
    invite_id: crate::musubi::MusubiInviteIdV1,
    invited_account: AccountId,
    role: MusubiPackageRoleV1,
    expires_at_height: u64,
    expected_governance_revision: u64,
});
impl_decode_musubi_instruction!(AcceptMusubiPackageMaintainerV1 {
    package: MusubiPackageIdV1,
    invite_id: crate::musubi::MusubiInviteIdV1,
    expected_governance_revision: u64,
});
impl_decode_musubi_instruction!(RevokeMusubiPackageMaintainerInvitationV1 {
    package: MusubiPackageIdV1,
    invite_id: crate::musubi::MusubiInviteIdV1,
    expected_governance_revision: u64,
});
impl_decode_musubi_instruction!(SetMusubiPackageMaintainerRoleV1 {
    package: MusubiPackageIdV1,
    account: AccountId,
    role: MusubiPackageRoleV1,
    expected_governance_revision: u64,
});
impl_decode_musubi_instruction!(RemoveMusubiPackageMaintainerV1 {
    package: MusubiPackageIdV1,
    account: AccountId,
    expected_governance_revision: u64,
});
impl_decode_musubi_instruction!(RegisterMusubiAliasV1 {
    alias: MusubiAliasNameV1,
    target: MusubiPackageIdV1,
    expected_pricing_revision: u64,
});
impl_decode_musubi_instruction!(RecoverMusubiPackageV1 {
    decision: MusubiGovernanceDecisionV1,
    package: MusubiPackageIdV1,
    owners: Vec<AccountId>,
    expected_governance_revision: u64,
});
impl_decode_musubi_instruction!(RetargetMusubiAliasV1 {
    decision: MusubiGovernanceDecisionV1,
    alias: MusubiAliasNameV1,
    target: MusubiPackageIdV1,
    expected_history_revision: u64,
});
impl_decode_musubi_instruction!(SetMusubiArtifactTakedownV1 {
    decision: MusubiGovernanceDecisionV1,
    release: MusubiReleaseIdV1,
    reason: MusubiReasonV1,
    expected_artifact_governance_revision: u64,
});
impl_decode_musubi_instruction!(SetMusubiRegistryPolicyV1 {
    decision: MusubiGovernanceDecisionV1,
    policy: MusubiRegistryPolicyV1,
    expected_policy_revision: u64,
});
impl_decode_musubi_instruction!(AssertMusubiReleaseDigestV1 {
    release: MusubiReleaseIdV1,
    expected_digest: MusubiReleaseDigestV1,
});

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        musubi::{
            ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiContentDigestV1,
            MusubiKotodamaEditionV1, MusubiPackageScopeV1, MusubiRegistrySnapshotV1,
            MusubiReleaseManifestV1, MusubiReleaseMetadataV1, MusubiResolutionProofV1,
            MusubiVerificationLockV1,
        },
        nexus::DataSpaceId,
    };

    fn package() -> MusubiPackageIdV1 {
        MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "math".parse().expect("package name"),
        )
    }

    fn release() -> MusubiReleaseIdV1 {
        MusubiReleaseIdV1::new(package(), "1.2.3".parse().expect("version"))
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    #[test]
    fn exact_digest_and_reversible_yank_roundtrip() {
        assert_slice_roundtrip(AssertMusubiReleaseDigestV1::new(
            release(),
            MusubiReleaseDigestV1::new([0x33; 32]),
        ));
        assert_slice_roundtrip(SetMusubiReleaseYankV1::new(
            release(),
            false,
            "restored".parse().expect("reason"),
            4,
        ));
    }

    #[test]
    fn pending_invitation_revoke_roundtrips() {
        assert_slice_roundtrip(RevokeMusubiPackageMaintainerInvitationV1 {
            package: package(),
            invite_id: crate::musubi::MusubiInviteIdV1::new([0x34; 32]),
            expected_governance_revision: 9,
        });
    }

    #[test]
    fn publish_namespace_is_explicit_in_wire_roundtrip() {
        let release = release();
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let publication = MusubiPublicationV1 {
            manifest: MusubiReleaseManifestV1 {
                release,
                edition: MusubiKotodamaEditionV1::V1,
                abi: MusubiAbiBindingV1::new([0x41; 32]).expect("ABI"),
                dependencies: Vec::new(),
                exports: Vec::new(),
                interface_digest: MusubiContentDigestV1::new([0x42; 32]),
                metadata: MusubiReleaseMetadataV1::default(),
                archive_id: ArchiveId::new([0x43; 32]),
                verification_lock_digest: lock.digest(),
            },
            resolution: MusubiResolutionProofV1 {
                snapshot: MusubiRegistrySnapshotV1 {
                    finalized_height: 7,
                    finalized_block_hash: [0x44; 32],
                    index_revision: 3,
                },
                lock,
            },
        };
        let instruction = PublishMusubiReleaseV1::new(
            "universal".parse().expect("namespace"),
            publication,
            None,
            5,
            None,
        );
        assert_slice_roundtrip(instruction);
    }

    #[test]
    fn legacy_wire_identifiers_are_not_reused() {
        let ids = [
            RegisterMusubiNamespaceBindingV1::WIRE_ID,
            RegisterMusubiArchiveV1::WIRE_ID,
            AddMusubiArchiveLocationV1::WIRE_ID,
            RetireMusubiArchiveLocationV1::WIRE_ID,
            PublishMusubiReleaseV1::WIRE_ID,
            SetMusubiReleaseYankV1::WIRE_ID,
            SetMusubiPackageMetadataV1::WIRE_ID,
            InviteMusubiPackageMaintainerV1::WIRE_ID,
            AcceptMusubiPackageMaintainerV1::WIRE_ID,
            RevokeMusubiPackageMaintainerInvitationV1::WIRE_ID,
            SetMusubiPackageMaintainerRoleV1::WIRE_ID,
            RemoveMusubiPackageMaintainerV1::WIRE_ID,
            RegisterMusubiAliasV1::WIRE_ID,
            RecoverMusubiPackageV1::WIRE_ID,
            RetargetMusubiAliasV1::WIRE_ID,
            SetMusubiArtifactTakedownV1::WIRE_ID,
            SetMusubiRegistryPolicyV1::WIRE_ID,
            AssertMusubiReleaseDigestV1::WIRE_ID,
        ];
        let unique = ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), ids.len());
        assert!(ids.iter().all(|id| id.contains(".v1.")));
        assert!(!ids.contains(&"iroha.musubi.release.publish"));
    }

    #[cfg(feature = "json")]
    #[test]
    fn v1_instruction_json_is_serializable_and_rejects_unknown_secret_fields() {
        let instruction = SetMusubiReleaseYankV1::new(
            release(),
            true,
            "security review".parse().expect("reason"),
            7,
        );
        let canonical = norito::json::to_json(&instruction)
            .expect("canonical Musubi V1 instruction JSON encodes");
        assert_eq!(
            norito::json::from_json::<SetMusubiReleaseYankV1>(&canonical)
                .expect("canonical Musubi V1 instruction JSON decodes"),
            instruction
        );

        let hostile = canonical.replacen('{', "{\"private_key\":\"must-not-be-accepted\",", 1);
        assert_ne!(hostile, canonical);
        assert!(
            norito::json::from_json::<SetMusubiReleaseYankV1>(&hostile).is_err(),
            "Musubi V1 instruction JSON must reject unknown secret-bearing fields"
        );
    }
}
