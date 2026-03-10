//! This module contains filters for data events.
//!
//! For each event in [`super::events`], there's a corresponding filter type in this module. It can filter the events by origin id and event type.
//!
//! Event types are filtered with an `EventSet` type, allowing to filter for multiple event types at once.

use std::fmt::Debug;

pub use bridge_filters_model::BridgeEventFilter;
use getset::Getters;
use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum DataEventFilter {
        /// Matches any data events ([`DataEvent`])
        Any,
        /// Matches [`PeerEvent`]s
        Peer(PeerEventFilter),
        /// Matches [`DomainEvent`]s
        Domain(DomainEventFilter),
        /// Matches [`AccountEvent`]s
        Account(AccountEventFilter),
        /// Matches [`AssetEvent`]s
        Asset(AssetEventFilter),
        /// Matches [`AssetDefinitionEvent`]s
        AssetDefinition(AssetDefinitionEventFilter),
        /// Matches [`NftEvent`]s
        Nft(NftEventFilter),
        /// Matches [`TriggerEvent`]s
        Trigger(TriggerEventFilter),
        /// Matches [`RoleEvent`]s
        Role(RoleEventFilter),
        /// Matches [`ConfigurationEvent`]s
        Configuration(ConfigurationEventFilter),
        /// Matches [`ExecutorEvent`]s
        Executor(ExecutorEventFilter),
        /// Matches proof verification events
        Proof(ProofEventFilter),
        /// Matches confidential asset events
        Confidential(ConfidentialEventFilter),
        /// Matches verifying key registry lifecycle events
        VerifyingKey(VerifyingKeyEventFilter),
        /// Matches runtime upgrade lifecycle events
        RuntimeUpgrade(RuntimeUpgradeEventFilter),
        /// Matches resolver directory governance events
        Soradns(SoradnsDirectoryEventFilter),
        /// Matches `SoraFS` gateway compliance events
        Sorafs(SorafsGatewayEventFilter),
        /// Matches Space Directory manifest lifecycle events
        SpaceDirectory(SpaceDirectoryEventFilter),
        /// Matches offline settlement lifecycle events
        Offline(OfflineTransferEventFilter),
        /// Matches oracle feed lifecycle events
        Oracle(OracleEventFilter),
        /// Matches viral incentive lifecycle events
        Social(SocialEventFilter),
        /// Matches [`BridgeEvent`]s
        Bridge(BridgeEventFilter),
        /// Matches governance lifecycle events
        #[cfg(feature = "governance")]
        Governance(GovernanceEventFilter),
    }

    /// An event filter for [`super::proof::ProofEvent`] values.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct ProofEventFilter {
        /// If specified, matches only events for this proof id
        pub(super) id_matcher: Option<crate::proof::ProofId>,
        /// Matches only events from this set
        pub(super) event_set: super::proof::ProofEventSet,
    }

    /// An event filter for [`super::confidential::ConfidentialEvent`] values.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct ConfidentialEventFilter {
        /// If specified, matches only events for this asset definition id
        pub(super) asset_matcher: Option<AssetDefinitionId>,
        /// Matches only events from this set
        pub(super) event_set: super::confidential::ConfidentialEventSet,
    }

    /// An event filter for [`super::verifying_keys::VerifyingKeyEvent`] values.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct VerifyingKeyEventFilter {
        /// If specified, matches only events for this verifying key id (backend + name)
        pub(super) id_matcher: Option<crate::proof::VerifyingKeyId>,
        /// Matches only events from this set
        pub(super) event_set: super::verifying_keys::VerifyingKeyEventSet,
    }

    /// An event filter for [`super::runtime_upgrade::RuntimeUpgradeEvent`] values.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct RuntimeUpgradeEventFilter {
        /// If specified, matches only events for this runtime upgrade id
        pub(super) id_matcher: Option<crate::runtime::RuntimeUpgradeId>,
        /// Matches only events from this set
        pub(super) event_set: super::runtime_upgrade::RuntimeUpgradeEventSet,
    }

    /// An event filter for viral incentive lifecycle events.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SocialEventFilter {
        /// If specified, matches only events for this binding hash.
        pub(super) binding_matcher: Option<crate::oracle::KeyedHash>,
    }

    /// An event filter for [`super::soradns::SoradnsDirectoryEvent`] values.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SoradnsDirectoryEventFilter {
        /// If specified, matches only events for this directory identifier.
        pub(super) directory_matcher: Option<crate::soradns::DirectoryId>,
        /// If specified, matches only events for this resolver identifier.
        pub(super) resolver_matcher: Option<crate::soradns::ResolverId>,
        /// Matches only events from this set.
        pub(super) event_set: super::soradns::SoradnsDirectoryEventSet,
    }

    /// An event filter for [`super::sorafs::SorafsGatewayEvent`] values.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SorafsGatewayEventFilter {
        /// If specified, matches only events for this provider id.
        pub(super) provider_matcher: Option<crate::sorafs::capacity::ProviderId>,
        /// If specified, matches only events for this manifest digest.
        pub(super) manifest_digest_matcher: Option<crate::sorafs::pin_registry::ManifestDigest>,
        /// If specified, matches only events for this policy classification.
        pub(super) policy_matcher: Option<super::sorafs::SorafsGarPolicy>,
        /// If specified, matches only events for this detailed outcome.
        pub(super) detail_matcher: Option<super::sorafs::SorafsGarPolicyDetail>,
        /// Matches only events from this set.
        pub(super) event_set: super::sorafs::SorafsGatewayEventSet,
    }

    /// Filter for Space Directory manifest lifecycle events.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SpaceDirectoryEventFilter {
        /// If specified, matches only events originating from this dataspace.
        pub(super) dataspace_matcher: Option<crate::nexus::DataSpaceId>,
        /// If specified, matches only events associated with this UAID.
        pub(super) uaid_matcher: Option<crate::nexus::UniversalAccountId>,
        /// Matches only events from this set.
        pub(super) event_set: super::space_directory::SpaceDirectoryEventSet,
    }

    #[cfg(feature = "governance")]
    /// An event filter for [`super::governance::GovernanceEvent`] values.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct GovernanceEventFilter {
        /// If specified, matches only events for this proposal id (`Proposal*` variants)
        pub(super) proposal_id: Option<[u8; 32]>,
        /// If specified, matches only events for this referendum id (`LockUpdated` variant)
        pub(super) referendum_id: Option<String>,
        /// Matches only events from this set
        pub(super) event_set: super::governance::GovernanceEventSet,
    }

    /// An event filter for [`PeerEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct PeerEventFilter {
        /// If specified matches only events originating from this peer
        pub(super) id_matcher: Option<super::PeerId>,
        /// Matches only event from this set
        pub(super) event_set: PeerEventSet,
    }

    /// An event filter for [`DomainEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct DomainEventFilter {
        /// If specified matches only events originating from this domain
        pub(super) id_matcher: Option<super::DomainId>,
        /// Matches only event from this set
        pub(super) event_set: DomainEventSet,
    }

    /// An event filter for [`AccountEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AccountEventFilter {
        /// If specified matches only events originating from this account
        pub(super) id_matcher: Option<super::AccountId>,
        /// Matches only event from this set
        pub(super) event_set: AccountEventSet,
    }

    /// An event filter for [`AssetEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetEventFilter {
        /// If specified matches only events originating from this asset
        pub(super) id_matcher: Option<super::AssetId>,
        /// Matches only event from this set
        pub(super) event_set: AssetEventSet,
    }

    /// An event filter for [`AssetDefinitionEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetDefinitionEventFilter {
        /// If specified matches only events originating from this asset definition
        pub(super) id_matcher: Option<super::AssetDefinitionId>,
        /// Matches only event from this set
        pub(super) event_set: AssetDefinitionEventSet,
    }

    /// An event filter for [`NftEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct NftEventFilter {
        /// If specified matches only events originating from this NFT
        pub(super) id_matcher: Option<NftId>,
        /// Matches only event from this set
        pub(super) event_set: NftEventSet,
    }

    /// An event filter for [`TriggerEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct TriggerEventFilter {
        /// If specified matches only events originating from this trigger
        pub(super) id_matcher: Option<super::TriggerId>,
        /// Matches only event from this set
        pub(super) event_set: TriggerEventSet,
    }

    /// An event filter for [`RoleEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct RoleEventFilter {
        /// If specified matches only events originating from this role
        pub(super) id_matcher: Option<super::RoleId>,
        /// Matches only event from this set
        pub(super) event_set: RoleEventSet,
    }

    /// An event filter for [`ConfigurationEvent`]s
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct ConfigurationEventFilter {
        /// Matches only event from this set
        pub(super) event_set: ConfigurationEventSet,
    }

    /// An event filter for [`ExecutorEvent`].
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct ExecutorEventFilter {
        // executor is a global entity, so no id here
        /// Matches only event from this set
        pub(super) event_set: ExecutorEventSet,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    DataEventFilter,
    ProofEventFilter,
    VerifyingKeyEventFilter,
    RuntimeUpgradeEventFilter,
    PeerEventFilter,
    DomainEventFilter,
    AccountEventFilter,
    AssetEventFilter,
    AssetDefinitionEventFilter,
    NftEventFilter,
    TriggerEventFilter,
    RoleEventFilter,
    ConfigurationEventFilter,
    ExecutorEventFilter,
);

#[cfg(all(feature = "json", feature = "governance"))]
impl_json_via_norito_bytes!(GovernanceEventFilter);

impl PeerEventFilter {
    /// Creates a new [`PeerEventFilter`] accepting all [`PeerEvent`]s.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: PeerEventSet::all(),
        }
    }

    /// Modifies a [`PeerEventFilter`] to accept only [`PeerEvent`]s originating from ids matching `id_matcher`.
    #[must_use]
    pub fn for_peer(mut self, id_matcher: PeerId) -> Self {
        self.id_matcher = Some(id_matcher);
        self
    }

    /// Modifies a [`PeerEventFilter`] to accept only [`PeerEvent`]s of types contained in `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: PeerEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl ProofEventFilter {
    /// Creates a new [`ProofEventFilter`] accepting all [`super::proof::ProofEvent`] values.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: super::proof::ProofEventSet::all(),
        }
    }

    /// Filter by proof identifier.
    #[must_use]
    pub fn for_proof(mut self, id: crate::proof::ProofId) -> Self {
        self.id_matcher = Some(id);
        self
    }

    /// Filter by event kinds.
    #[must_use]
    pub const fn for_events(mut self, event_set: super::proof::ProofEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl ConfidentialEventFilter {
    /// Creates a new [`ConfidentialEventFilter`] accepting all [`ConfidentialEvent`]s.
    pub const fn new() -> Self {
        Self {
            asset_matcher: None,
            event_set: super::confidential::ConfidentialEventSet::all(),
        }
    }

    /// Filter by asset definition identifier.
    #[must_use]
    pub fn for_asset_definition(mut self, id: AssetDefinitionId) -> Self {
        self.asset_matcher = Some(id);
        self
    }

    /// Filter by confidential event kinds.
    #[must_use]
    pub const fn for_events(
        mut self,
        event_set: super::confidential::ConfidentialEventSet,
    ) -> Self {
        self.event_set = event_set;
        self
    }
}

impl VerifyingKeyEventFilter {
    /// Creates a new [`VerifyingKeyEventFilter`] accepting all [`super::verifying_keys::VerifyingKeyEvent`] values.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: super::verifying_keys::VerifyingKeyEventSet::all(),
        }
    }

    /// Filter by verifying key identifier.
    #[must_use]
    pub fn for_verifying_key(mut self, id: crate::proof::VerifyingKeyId) -> Self {
        self.id_matcher = Some(id);
        self
    }

    /// Filter by event kinds.
    #[must_use]
    pub const fn for_events(
        mut self,
        event_set: super::verifying_keys::VerifyingKeyEventSet,
    ) -> Self {
        self.event_set = event_set;
        self
    }
}

impl RuntimeUpgradeEventFilter {
    /// Creates a new [`RuntimeUpgradeEventFilter`] accepting all runtime upgrade events.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: super::runtime_upgrade::RuntimeUpgradeEventSet::all(),
        }
    }

    /// Filter by runtime upgrade identifier.
    #[must_use]
    pub fn for_runtime(mut self, id: crate::runtime::RuntimeUpgradeId) -> Self {
        self.id_matcher = Some(id);
        self
    }

    /// Filter by runtime upgrade event kinds.
    #[must_use]
    pub const fn for_events(
        mut self,
        event_set: super::runtime_upgrade::RuntimeUpgradeEventSet,
    ) -> Self {
        self.event_set = event_set;
        self
    }
}

impl SorafsGatewayEventFilter {
    /// Creates a new [`SorafsGatewayEventFilter`] accepting all gateway events.
    pub const fn new() -> Self {
        Self {
            provider_matcher: None,
            manifest_digest_matcher: None,
            policy_matcher: None,
            detail_matcher: None,
            event_set: super::sorafs::SorafsGatewayEventSet::all(),
        }
    }

    /// Filter by provider identifier.
    #[must_use]
    pub fn for_provider(mut self, provider: crate::sorafs::capacity::ProviderId) -> Self {
        self.provider_matcher = Some(provider);
        self
    }

    /// Filter by manifest digest.
    #[must_use]
    pub fn for_manifest_digest(
        mut self,
        digest: crate::sorafs::pin_registry::ManifestDigest,
    ) -> Self {
        self.manifest_digest_matcher = Some(digest);
        self
    }

    /// Filter by policy classification.
    #[must_use]
    pub fn for_policy(mut self, policy: super::sorafs::SorafsGarPolicy) -> Self {
        self.policy_matcher = Some(policy);
        self
    }

    /// Filter by detailed outcome.
    #[must_use]
    pub fn for_detail(mut self, detail: super::sorafs::SorafsGarPolicyDetail) -> Self {
        self.detail_matcher = Some(detail);
        self
    }

    /// Filter by event variant set.
    #[must_use]
    pub const fn for_events(mut self, event_set: super::sorafs::SorafsGatewayEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl SoradnsDirectoryEventFilter {
    /// Creates a new [`SoradnsDirectoryEventFilter`] accepting all directory events.
    pub const fn new() -> Self {
        Self {
            directory_matcher: None,
            resolver_matcher: None,
            event_set: super::soradns::SoradnsDirectoryEventSet::all(),
        }
    }

    /// Constrains the filter to a specific directory identifier.
    #[must_use]
    pub const fn for_directory(mut self, directory_id: crate::soradns::DirectoryId) -> Self {
        self.directory_matcher = Some(directory_id);
        self
    }

    /// Constrains the filter to a specific resolver identifier.
    #[must_use]
    pub const fn for_resolver(mut self, resolver_id: crate::soradns::ResolverId) -> Self {
        self.resolver_matcher = Some(resolver_id);
        self
    }

    /// Restricts the filter to the supplied set of events.
    #[must_use]
    pub const fn for_events(mut self, event_set: super::soradns::SoradnsDirectoryEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl SpaceDirectoryEventFilter {
    /// Creates a new [`SpaceDirectoryEventFilter`] accepting all events.
    pub const fn new() -> Self {
        Self {
            dataspace_matcher: None,
            uaid_matcher: None,
            event_set: super::space_directory::SpaceDirectoryEventSet::all(),
        }
    }

    /// Filter by dataspace identifier.
    #[must_use]
    pub fn for_dataspace(mut self, dataspace: crate::nexus::DataSpaceId) -> Self {
        self.dataspace_matcher = Some(dataspace);
        self
    }

    /// Filter by UAID.
    #[must_use]
    pub fn for_uaid(mut self, uaid: crate::nexus::UniversalAccountId) -> Self {
        self.uaid_matcher = Some(uaid);
        self
    }

    /// Filter by event variants.
    #[must_use]
    pub const fn for_events(
        mut self,
        event_set: super::space_directory::SpaceDirectoryEventSet,
    ) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for RuntimeUpgradeEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for VerifyingKeyEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ProofEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ConfidentialEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SorafsGatewayEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SoradnsDirectoryEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for SpaceDirectoryEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for OracleEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// An event filter for [`super::offline::OfflineTransferEvent`] values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct OfflineTransferEventFilter {
    /// Optional bundle identifier matcher.
    pub(super) bundle_matcher: Option<iroha_crypto::Hash>,
    /// Optional receiver matcher (applies only to settled events).
    pub(super) receiver_matcher: Option<crate::account::AccountId>,
    /// Optional platform policy matcher (applies only to settled events with platform snapshots).
    pub(super) platform_policy_matcher: Option<crate::offline::AndroidIntegrityPolicy>,
    /// Matched event-set.
    pub(super) event_set: super::offline::OfflineTransferEventSet,
}

/// Filter for oracle feed aggregation events.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct OracleEventFilter {
    /// Optional feed identifier matcher.
    pub(super) feed_matcher: Option<crate::oracle::FeedId>,
    /// Matched event-set.
    pub(super) event_set: super::oracle::OracleEventSet,
}

impl OracleEventFilter {
    /// Creates a new [`OracleEventFilter`] accepting all events.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            feed_matcher: None,
            event_set: super::oracle::OracleEventSet::all(),
        }
    }

    /// Restricts matches to the provided feed identifier.
    #[must_use]
    pub fn for_feed(mut self, feed_id: crate::oracle::FeedId) -> Self {
        self.feed_matcher = Some(feed_id);
        self
    }

    /// Restricts matches to the provided oracle event set.
    #[must_use]
    pub const fn for_events(mut self, event_set: super::oracle::OracleEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl OfflineTransferEventFilter {
    /// Creates a new [`OfflineTransferEventFilter`] accepting all events.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bundle_matcher: None,
            receiver_matcher: None,
            platform_policy_matcher: None,
            event_set: super::offline::OfflineTransferEventSet::all(),
        }
    }

    /// Restricts matches to the provided bundle identifier.
    #[must_use]
    pub fn for_bundle(mut self, bundle_id: iroha_crypto::Hash) -> Self {
        self.bundle_matcher = Some(bundle_id);
        self
    }

    /// Restricts matches to settled events targeting the given receiver.
    #[must_use]
    pub fn for_receiver(mut self, receiver: crate::account::AccountId) -> Self {
        self.receiver_matcher = Some(receiver);
        self
    }

    /// Restricts matches to settled events capturing the provided platform token policy.
    #[must_use]
    pub fn for_platform_policy(mut self, policy: crate::offline::AndroidIntegrityPolicy) -> Self {
        self.platform_policy_matcher = Some(policy);
        self
    }

    /// Restricts matches to the provided event-set.
    #[must_use]
    pub const fn for_events(mut self, event_set: super::offline::OfflineTransferEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for OfflineTransferEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl SocialEventFilter {
    /// Creates a new [`SocialEventFilter`] accepting all viral incentive events.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            binding_matcher: None,
        }
    }

    /// Restricts matches to the provided binding hash.
    #[must_use]
    pub fn for_binding(mut self, binding: crate::oracle::KeyedHash) -> Self {
        self.binding_matcher = Some(binding);
        self
    }
}

impl Default for SocialEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for PeerEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for ProofEventFilter {
    type Event = super::proof::ProofEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        let id_ok = self
            .id_matcher
            .as_ref()
            .is_none_or(|id_matcher| match event {
                super::proof::ProofEvent::Verified(v) => id_matcher == &v.id,
                super::proof::ProofEvent::Rejected(r) => id_matcher == &r.id,
                super::proof::ProofEvent::Pruned(p) => {
                    p.removed.iter().any(|rid| id_matcher == rid)
                }
            });

        id_ok && self.event_set.matches(event)
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for ConfidentialEventFilter {
    type Event = super::confidential::ConfidentialEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        let asset_ok = self.asset_matcher.as_ref().is_none_or(|id| match event {
            super::confidential::ConfidentialEvent::Shielded(e) => id == &e.asset_definition,
            super::confidential::ConfidentialEvent::Transferred(e) => id == &e.asset_definition,
            super::confidential::ConfidentialEvent::Unshielded(e) => id == &e.asset_definition,
        });
        asset_ok && self.event_set.matches(event)
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for VerifyingKeyEventFilter {
    type Event = super::verifying_keys::VerifyingKeyEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        let id_ok = self
            .id_matcher
            .as_ref()
            .is_none_or(|id_matcher| match event {
                super::verifying_keys::VerifyingKeyEvent::Registered(e) => id_matcher == &e.id,
                super::verifying_keys::VerifyingKeyEvent::Updated(e) => id_matcher == &e.id,
            });
        id_ok && self.event_set.matches(event)
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for RuntimeUpgradeEventFilter {
    type Event = super::runtime_upgrade::RuntimeUpgradeEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        let id_ok = self
            .id_matcher
            .as_ref()
            .is_none_or(|id_matcher| match event {
                super::runtime_upgrade::RuntimeUpgradeEvent::Proposed(e) => id_matcher == &e.id,
                super::runtime_upgrade::RuntimeUpgradeEvent::Activated(e) => id_matcher == &e.id,
                super::runtime_upgrade::RuntimeUpgradeEvent::Canceled(e) => id_matcher == &e.id,
            });
        id_ok && self.event_set.matches(event)
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for OracleEventFilter {
    type Event = super::oracle::OracleEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        let event_feed = match event {
            super::oracle::OracleEvent::FeedProcessed(record) => Some(&record.event.feed_id),
            super::oracle::OracleEvent::PenaltyApplied(penalty) => Some(&penalty.feed_id),
            super::oracle::OracleEvent::RewardApplied(reward) => Some(&reward.feed_id),
            super::oracle::OracleEvent::DisputeOpened(dispute)
            | super::oracle::OracleEvent::DisputeResolved(dispute) => Some(&dispute.feed_id),
            super::oracle::OracleEvent::ChangeProposed(change) => Some(&change.feed_id),
            super::oracle::OracleEvent::ChangeStageUpdated(_)
            | super::oracle::OracleEvent::TwitterBindingRevoked(_) => None,
            super::oracle::OracleEvent::TwitterBindingRecorded(record) => {
                Some(&record.record.feed_id)
            }
        };

        if let Some(feed_id) = self.feed_matcher.as_ref()
            && event_feed != Some(feed_id)
        {
            return false;
        }

        self.event_set.matches(event)
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for SoradnsDirectoryEventFilter {
    type Event = super::soradns::SoradnsDirectoryEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        if !self.event_set.matches(event) {
            return false;
        }

        if let Some(directory_id) = self.directory_matcher.as_ref() {
            let matches_directory = match event {
                super::soradns::SoradnsDirectoryEvent::DraftSubmitted(payload) => {
                    &payload.directory_id == directory_id
                }
                super::soradns::SoradnsDirectoryEvent::Published(payload) => {
                    &payload.directory_id == directory_id
                }
                _ => false,
            };
            if !matches_directory {
                return false;
            }
        }

        if let Some(resolver_id) = self.resolver_matcher.as_ref() {
            let matches_resolver = match event {
                super::soradns::SoradnsDirectoryEvent::Revoked(payload) => {
                    &payload.resolver_id == resolver_id
                }
                super::soradns::SoradnsDirectoryEvent::Unrevoked(payload) => {
                    &payload.resolver_id == resolver_id
                }
                _ => false,
            };
            if !matches_resolver {
                return false;
            }
        }

        true
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for SorafsGatewayEventFilter {
    type Event = super::sorafs::SorafsGatewayEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        if !self.event_set.matches(event) {
            return false;
        }

        match event {
            super::sorafs::SorafsGatewayEvent::GarViolation(payload) => {
                let provider_ok = self
                    .provider_matcher
                    .as_ref()
                    .is_none_or(|expected| payload.provider_id.as_ref() == Some(expected));
                if !provider_ok {
                    return false;
                }

                let digest_ok = self
                    .manifest_digest_matcher
                    .as_ref()
                    .is_none_or(|expected| payload.manifest_digest.as_ref() == Some(expected));
                if !digest_ok {
                    return false;
                }

                let policy_ok = self
                    .policy_matcher
                    .as_ref()
                    .is_none_or(|expected| &payload.policy == expected);
                if !policy_ok {
                    return false;
                }

                self.detail_matcher
                    .as_ref()
                    .is_none_or(|expected| &payload.detail == expected)
            }
            super::sorafs::SorafsGatewayEvent::DealUsage(payload) => {
                if self.manifest_digest_matcher.is_some()
                    || self.policy_matcher.is_some()
                    || self.detail_matcher.is_some()
                {
                    return false;
                }
                self.provider_matcher
                    .as_ref()
                    .is_none_or(|expected| &payload.provider_id == expected)
            }
            super::sorafs::SorafsGatewayEvent::DealSettlement(payload) => {
                if self.manifest_digest_matcher.is_some()
                    || self.policy_matcher.is_some()
                    || self.detail_matcher.is_some()
                {
                    return false;
                }
                self.provider_matcher
                    .as_ref()
                    .is_none_or(|expected| &payload.record.provider_id == expected)
            }
            super::sorafs::SorafsGatewayEvent::ProofHealth(payload) => {
                if self.manifest_digest_matcher.is_some()
                    || self.policy_matcher.is_some()
                    || self.detail_matcher.is_some()
                {
                    return false;
                }
                self.provider_matcher
                    .as_ref()
                    .is_none_or(|expected| &payload.provider_id == expected)
            }
        }
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for SpaceDirectoryEventFilter {
    type Event = super::space_directory::SpaceDirectoryEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        if !self.event_set.matches(event) {
            return false;
        }

        let (dataspace, uaid) = match event {
            super::space_directory::SpaceDirectoryEvent::ManifestActivated(payload) => {
                (payload.dataspace, payload.uaid)
            }
            super::space_directory::SpaceDirectoryEvent::ManifestExpired(payload) => {
                (payload.dataspace, payload.uaid)
            }
            super::space_directory::SpaceDirectoryEvent::ManifestRevoked(payload) => {
                (payload.dataspace, payload.uaid)
            }
        };

        if let Some(expected) = self.dataspace_matcher
            && dataspace != expected
        {
            return false;
        }

        if let Some(expected) = self.uaid_matcher
            && uaid != expected
        {
            return false;
        }

        true
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for OfflineTransferEventFilter {
    type Event = super::offline::OfflineTransferEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        if !self.event_set.matches(event) {
            return false;
        }

        if let Some(expected_bundle) = &self.bundle_matcher {
            let actual_bundle = match event {
                super::offline::OfflineTransferEvent::Settled(payload) => &payload.bundle_id,
                super::offline::OfflineTransferEvent::Archived(payload) => &payload.bundle_id,
                super::offline::OfflineTransferEvent::Pruned(payload) => &payload.bundle_id,
                super::offline::OfflineTransferEvent::RevocationImported(_)
                | super::offline::OfflineTransferEvent::AllowanceReclaimed(_) => return false,
            };
            if actual_bundle != expected_bundle {
                return false;
            }
        }

        if let Some(expected_receiver) = &self.receiver_matcher {
            match event {
                super::offline::OfflineTransferEvent::Settled(payload) => {
                    if &payload.receiver != expected_receiver {
                        return false;
                    }
                }
                super::offline::OfflineTransferEvent::Archived(_)
                | super::offline::OfflineTransferEvent::Pruned(_)
                | super::offline::OfflineTransferEvent::RevocationImported(_)
                | super::offline::OfflineTransferEvent::AllowanceReclaimed(_) => return false,
            }
        }

        if let Some(expected_policy) = self.platform_policy_matcher {
            match event {
                super::offline::OfflineTransferEvent::Settled(payload) => {
                    let Some(snapshot) = payload.platform_snapshot.as_ref() else {
                        return false;
                    };
                    if snapshot.policy() != Some(expected_policy) {
                        return false;
                    }
                }
                super::offline::OfflineTransferEvent::Archived(_)
                | super::offline::OfflineTransferEvent::Pruned(_)
                | super::offline::OfflineTransferEvent::RevocationImported(_)
                | super::offline::OfflineTransferEvent::AllowanceReclaimed(_) => return false,
            }
        }

        true
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for SocialEventFilter {
    type Event = super::social::SocialEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        let binding_hash = match event {
            super::social::SocialEvent::RewardPaid(ev) => &ev.binding_hash,
            super::social::SocialEvent::EscrowCreated(ev) => &ev.escrow.binding_hash,
            super::social::SocialEvent::EscrowReleased(ev) => &ev.escrow.binding_hash,
            super::social::SocialEvent::EscrowCancelled(ev) => &ev.escrow.binding_hash,
        };

        self.binding_matcher
            .as_ref()
            .is_none_or(|expected| expected == binding_hash)
    }
}

#[cfg(feature = "transparent_api")]
impl EventFilter for PeerEventFilter {
    type Event = super::PeerEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.id_matcher
            .as_ref()
            .is_none_or(|id| id == event.origin())
            && self.event_set.matches(event)
    }
}

impl DomainEventFilter {
    /// Creates a new [`DomainEventFilter`] accepting all [`DomainEvent`]s.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: DomainEventSet::all(),
        }
    }

    /// Modifies a [`DomainEventFilter`] to accept only [`DomainEvent`]s originating from ids matching `id_matcher`.
    #[must_use]
    pub fn for_domain(mut self, id_matcher: DomainId) -> Self {
        self.id_matcher = Some(id_matcher);
        self
    }

    /// Modifies a [`DomainEventFilter`] to accept only [`DomainEvent`]s of types contained in `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: DomainEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for DomainEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl EventFilter for DomainEventFilter {
    type Event = super::DomainEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.id_matcher
            .as_ref()
            .is_none_or(|id| id == event.origin())
            && self.event_set.matches(event)
    }
}

impl AccountEventFilter {
    /// Creates a new [`AccountEventFilter`] accepting all [`AccountEvent`]s.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: AccountEventSet::all(),
        }
    }

    /// Modifies a [`AccountEventFilter`] to accept only [`AccountEvent`]s originating from ids matching `id_matcher`.
    #[must_use]
    pub fn for_account(mut self, id_matcher: AccountId) -> Self {
        self.id_matcher = Some(id_matcher);
        self
    }

    /// Modifies a [`AccountEventFilter`] to accept only [`AccountEvent`]s of types contained in `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: AccountEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for AccountEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for AccountEventFilter {
    type Event = super::AccountEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.id_matcher
            .as_ref()
            .is_none_or(|id| id == event.origin())
            && self.event_set.matches(event)
    }
}

impl AssetEventFilter {
    /// Creates a new [`AssetEventFilter`] accepting all [`AssetEvent`]s.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: AssetEventSet::all(),
        }
    }

    /// Modifies a [`AssetEventFilter`] to accept only [`AssetEvent`]s originating from ids matching `id_matcher`.
    #[must_use]
    pub fn for_asset(mut self, id_matcher: AssetId) -> Self {
        self.id_matcher = Some(id_matcher);
        self
    }

    /// Modifies a [`AssetEventFilter`] to accept only [`AssetEvent`]s of types contained in `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: AssetEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for AssetEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for AssetEventFilter {
    type Event = super::AssetEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.id_matcher
            .as_ref()
            .is_none_or(|id| id == event.origin())
            && self.event_set.matches(event)
    }
}

impl AssetDefinitionEventFilter {
    /// Creates a new [`AssetDefinitionEventFilter`] accepting all [`AssetDefinitionEvent`]s.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: AssetDefinitionEventSet::all(),
        }
    }

    /// Modifies a [`AssetDefinitionEventFilter`] to accept only [`AssetDefinitionEvent`]s originating from ids matching `id_matcher`.
    #[must_use]
    pub fn for_asset_definition(mut self, id_matcher: AssetDefinitionId) -> Self {
        self.id_matcher = Some(id_matcher);
        self
    }

    /// Modifies a [`AssetDefinitionEventFilter`] to accept only [`AssetDefinitionEvent`]s of types contained in `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: AssetDefinitionEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for AssetDefinitionEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for AssetDefinitionEventFilter {
    type Event = super::AssetDefinitionEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.id_matcher
            .as_ref()
            .is_none_or(|id| id == event.origin())
            && self.event_set.matches(event)
    }
}

impl NftEventFilter {
    /// Creates a new [`NftEventFilter`] accepting all [`NftEvent`]s.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: NftEventSet::all(),
        }
    }

    /// Modifies a [`NftEventFilter`] to accept only [`NftEvent`]s originating from ids matching `id_matcher`.
    #[must_use]
    pub fn for_nft(mut self, id_matcher: NftId) -> Self {
        self.id_matcher = Some(id_matcher);
        self
    }

    /// Modifies a [`NftEventFilter`] to accept only [`NftEvent`]s of types contained in `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: NftEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for NftEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl EventFilter for NftEventFilter {
    type Event = NftEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.id_matcher
            .as_ref()
            .is_none_or(|id| id == event.origin())
            && self.event_set.matches(event)
    }
}

impl TriggerEventFilter {
    /// Creates a new [`TriggerEventFilter`] accepting all [`TriggerEvent`]s.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: TriggerEventSet::all(),
        }
    }

    /// Modifies a [`TriggerEventFilter`] to accept only [`TriggerEvent`]s originating from ids matching `id_matcher`.
    #[must_use]
    pub fn for_trigger(mut self, id_matcher: TriggerId) -> Self {
        self.id_matcher = Some(id_matcher);
        self
    }

    /// Modifies a [`TriggerEventFilter`] to accept only [`TriggerEvent`]s of types matching `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: TriggerEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for TriggerEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for TriggerEventFilter {
    type Event = super::TriggerEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.id_matcher
            .as_ref()
            .is_none_or(|id| id == event.origin())
            && self.event_set.matches(event)
    }
}

impl RoleEventFilter {
    /// Creates a new [`RoleEventFilter`] accepting all [`RoleEvent`]s.
    pub const fn new() -> Self {
        Self {
            id_matcher: None,
            event_set: RoleEventSet::all(),
        }
    }

    /// Modifies a [`RoleEventFilter`] to accept only [`RoleEvent`]s originating from ids matching `id_matcher`.
    #[must_use]
    pub fn for_role(mut self, id_matcher: RoleId) -> Self {
        self.id_matcher = Some(id_matcher);
        self
    }

    /// Modifies a [`RoleEventFilter`] to accept only [`RoleEvent`]s of types matching `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: RoleEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for RoleEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for RoleEventFilter {
    type Event = super::RoleEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.id_matcher
            .as_ref()
            .is_none_or(|id| id == event.origin())
            && self.event_set.matches(event)
    }
}

impl ConfigurationEventFilter {
    /// Creates a new [`ConfigurationEventFilter`] accepting all [`ConfigurationEvent`]s.
    pub const fn new() -> Self {
        Self {
            event_set: ConfigurationEventSet::all(),
        }
    }

    /// Modifies a [`ConfigurationEventFilter`] to accept only [`ConfigurationEvent`]s of types matching `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: ConfigurationEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for ConfigurationEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for ConfigurationEventFilter {
    type Event = super::ConfigurationEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.event_set.matches(event)
    }
}

impl ExecutorEventFilter {
    /// Creates a new [`ExecutorEventFilter`] accepting all [`ExecutorEvent`]s.
    pub const fn new() -> Self {
        Self {
            event_set: ExecutorEventSet::all(),
        }
    }

    /// Modifies a [`ExecutorEventFilter`] to accept only [`ExecutorEvent`]s of types matching `event_set`.
    #[must_use]
    pub const fn for_events(mut self, event_set: ExecutorEventSet) -> Self {
        self.event_set = event_set;
        self
    }
}

impl Default for ExecutorEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for ExecutorEventFilter {
    type Event = super::ExecutorEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.event_set.matches(event)
    }
}

#[cfg(feature = "transparent_api")]
impl super::EventFilter for BridgeEventFilter {
    type Event = crate::events::data::events::bridge::BridgeEvent;

    fn matches(&self, event: &Self::Event) -> bool {
        self.matches_event(event)
    }
}

#[cfg(feature = "governance")]
impl GovernanceEventFilter {
    /// Creates a new [`GovernanceEventFilter`] accepting all governance events.
    pub fn new() -> Self {
        Self {
            proposal_id: None,
            referendum_id: None,
            event_set: super::governance::GovernanceEventSet::all(),
        }
    }

    /// Match only the provided event set.
    #[must_use]
    pub fn for_events(mut self, set: super::governance::GovernanceEventSet) -> Self {
        self.event_set = set;
        self
    }

    /// Filter by proposal id (applies to Proposal* variants only).
    #[must_use]
    pub fn for_proposal(mut self, id: [u8; 32]) -> Self {
        self.proposal_id = Some(id);
        self
    }

    /// Filter by referendum id (applies to `LockUpdated` only).
    #[must_use]
    pub fn for_referendum(mut self, rid: String) -> Self {
        self.referendum_id = Some(rid);
        self
    }
}

#[cfg(feature = "governance")]
impl Default for GovernanceEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "transparent_api")]
impl EventFilter for DataEventFilter {
    type Event = DataEvent;

    fn matches(&self, event: &DataEvent) -> bool {
        match (self, event) {
            (DataEventFilter::Any, _) => true,
            (DataEventFilter::Peer(filter), DataEvent::Peer(peer_event)) => {
                filter.matches(peer_event)
            }
            (DataEventFilter::Domain(filter), DataEvent::Domain(domain_event)) => {
                filter.matches(domain_event)
            }
            (
                DataEventFilter::Account(filter),
                DataEvent::Domain(DomainEvent::Account(account_event)),
            ) => filter.matches(account_event),
            (
                DataEventFilter::Asset(filter),
                DataEvent::Domain(DomainEvent::Account(AccountEvent::Asset(asset_event))),
            ) => filter.matches(asset_event),
            (
                DataEventFilter::AssetDefinition(filter),
                DataEvent::Domain(DomainEvent::AssetDefinition(asset_definition_event)),
            ) => filter.matches(asset_definition_event),
            (DataEventFilter::Nft(filter), DataEvent::Domain(DomainEvent::Nft(nft_event))) => {
                filter.matches(nft_event)
            }
            (DataEventFilter::Trigger(filter), DataEvent::Trigger(trigger_event)) => {
                filter.matches(trigger_event)
            }
            (DataEventFilter::Role(filter), DataEvent::Role(role_event)) => {
                filter.matches(role_event)
            }
            (
                DataEventFilter::Configuration(filter),
                DataEvent::Configuration(configuration_event),
            ) => filter.matches(configuration_event),
            (DataEventFilter::Executor(filter), DataEvent::Executor(executor_event)) => {
                filter.matches(executor_event)
            }
            (DataEventFilter::Proof(filter), DataEvent::Proof(proof_event)) => {
                filter.matches(proof_event)
            }
            (
                DataEventFilter::Confidential(filter),
                DataEvent::Confidential(confidential_event),
            ) => filter.matches(confidential_event),
            (
                DataEventFilter::VerifyingKey(filter),
                DataEvent::VerifyingKey(verifying_key_event),
            ) => filter.matches(verifying_key_event),
            (
                DataEventFilter::RuntimeUpgrade(filter),
                DataEvent::RuntimeUpgrade(runtime_upgrade_event),
            ) => filter.matches(runtime_upgrade_event),
            (DataEventFilter::Soradns(filter), DataEvent::Soradns(soradns_event)) => {
                filter.matches(soradns_event)
            }
            (DataEventFilter::Sorafs(filter), DataEvent::Sorafs(sorafs_event)) => {
                filter.matches(sorafs_event)
            }
            (DataEventFilter::SpaceDirectory(filter), DataEvent::SpaceDirectory(space_event)) => {
                filter.matches(space_event)
            }
            (DataEventFilter::Offline(filter), DataEvent::Offline(offline_event)) => {
                filter.matches(offline_event)
            }
            (DataEventFilter::Oracle(filter), DataEvent::Oracle(oracle_event)) => {
                filter.matches(oracle_event)
            }
            (DataEventFilter::Social(filter), DataEvent::Social(social_event)) => {
                filter.matches(social_event)
            }
            (DataEventFilter::Bridge(filter), DataEvent::Bridge(bridge_event)) => {
                filter.matches_event(bridge_event)
            }
            #[cfg(feature = "governance")]
            (DataEventFilter::Governance(filter), DataEvent::Governance(governance_event)) => {
                governance_matches(filter, governance_event)
            }
            _ => false,
        }
    }
}

#[cfg(all(feature = "transparent_api", feature = "governance"))]
fn governance_matches(
    filter: &GovernanceEventFilter,
    event: &super::governance::GovernanceEvent,
) -> bool {
    use super::governance::GovernanceEvent;

    if !filter.event_set.matches(event) {
        return false;
    }

    let proposal_matches =
        |candidate: &[u8; 32]| filter.proposal_id.as_ref().is_none_or(|id| candidate == id);

    let referendum_matches = |candidate: &str| {
        filter
            .referendum_id
            .as_ref()
            .is_none_or(|rid| candidate == rid)
    };

    match event {
        GovernanceEvent::ProposalSubmitted(p) => proposal_matches(&p.id),
        GovernanceEvent::ProposalEnacted(p) => proposal_matches(&p.id),
        GovernanceEvent::LockCreated(lock) => referendum_matches(&lock.referendum_id),
        GovernanceEvent::LockExtended(lock) => referendum_matches(&lock.referendum_id),
        GovernanceEvent::ProposalApproved(_)
        | GovernanceEvent::ProposalRejected(_)
        | GovernanceEvent::BallotAccepted(_)
        | GovernanceEvent::BallotRejected(_)
        | GovernanceEvent::ReferendumOpened(_)
        | GovernanceEvent::ReferendumClosed(_)
        | GovernanceEvent::LockUnlocked(_)
        | GovernanceEvent::CitizenRegistered(_)
        | GovernanceEvent::CitizenRevoked(_)
        | GovernanceEvent::CitizenServiceRecorded(_)
        | GovernanceEvent::CouncilPersisted(_)
        | GovernanceEvent::ParliamentSelected(_) => true,
        GovernanceEvent::ParliamentApprovalRecorded(ev) => proposal_matches(&ev.proposal_id),
        GovernanceEvent::LockSlashed(ev) => referendum_matches(&ev.referendum_id),
        GovernanceEvent::LockRestituted(ev) => referendum_matches(&ev.referendum_id),
    }
}

pub mod prelude {
    #[cfg(feature = "governance")]
    pub use super::GovernanceEventFilter;
    pub use super::{
        AccountEventFilter, AssetDefinitionEventFilter, AssetEventFilter, BridgeEventFilter,
        ConfidentialEventFilter, ConfigurationEventFilter, DataEventFilter, DomainEventFilter,
        ExecutorEventFilter, NftEventFilter, OfflineTransferEventFilter, OracleEventFilter,
        PeerEventFilter, ProofEventFilter, RoleEventFilter, SocialEventFilter,
        SoradnsDirectoryEventFilter, SorafsGatewayEventFilter, TriggerEventFilter,
        VerifyingKeyEventFilter,
    };
}
#[cfg(test)]
#[cfg(feature = "transparent_api")]
mod tests {
    use iroha_crypto::{Hash, KeyPair};
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::nexus::UniversalAccountId;

    #[test]
    #[cfg(feature = "transparent_api")]
    fn entity_scope() {
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let account_id = AccountId::new(KeyPair::random().into_parts().0);
        let definition_id: crate::asset::AssetDefinitionId = "rose#wonderland".parse().unwrap();
        let asset_id = AssetId::new(definition_id, account_id.clone());
        let domain_owner_id = AccountId::new(KeyPair::random().into_parts().0);

        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: domain_owner_id,
        };
        let account = Account::new(account_id.to_account_id(domain_id.clone())).into_account();
        let asset = Asset::new(asset_id.clone(), 0_u32);

        // Create three events with three levels of nesting
        // the first one is just a domain event
        // the second one is an account event with a domain event inside
        // the third one is an asset event with an account event with a domain event inside
        let domain_created = DomainEvent::Created(domain).into();
        let account_created = DomainEvent::Account(AccountEvent::Created(AccountCreated {
            account,
            domain: domain_id.clone(),
        }))
        .into();
        let asset_created =
            DomainEvent::Account(AccountEvent::Asset(AssetEvent::Created(asset))).into();

        // test how the differently nested filters with with the events
        let domain_filter = DataEventFilter::Domain(DomainEventFilter::new().for_domain(domain_id));
        let account_filter =
            DataEventFilter::Account(AccountEventFilter::new().for_account(account_id));
        let asset_filter = DataEventFilter::Asset(AssetEventFilter::new().for_asset(asset_id));

        // domain filter matches all of those, because all of those events happened in the same domain
        assert!(domain_filter.matches(&domain_created));
        assert!(domain_filter.matches(&account_created));
        assert!(domain_filter.matches(&asset_created));

        // account event does not match the domain created event, as it is not an account event
        assert!(!account_filter.matches(&domain_created));
        assert!(account_filter.matches(&account_created));
        assert!(account_filter.matches(&asset_created));

        // asset event matches only the domain->account->asset event
        assert!(!asset_filter.matches(&domain_created));
        assert!(!asset_filter.matches(&account_created));
        assert!(asset_filter.matches(&asset_created));
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn verifying_key_filter_matches_by_id() {
        use crate::{
            confidential::ConfidentialStatus,
            events::data::verifying_keys::{VerifyingKeyEvent, VerifyingKeyRegistered},
            proof::VerifyingKeyRecord,
            zk::BackendTag,
        };
        let id = crate::proof::VerifyingKeyId::new("halo2/ipa", "vk_test");
        let mut rec = VerifyingKeyRecord::new(
            1,
            "vk_transfer",
            BackendTag::Halo2IpaPasta,
            "pallas",
            [0u8; 32],
            [0u8; 32],
        );
        rec.status = ConfidentialStatus::Active;
        let ev = DataEvent::VerifyingKey(VerifyingKeyEvent::Registered(VerifyingKeyRegistered {
            id: id.clone(),
            record: rec,
        }));
        let ok_filter = DataEventFilter::VerifyingKey(
            VerifyingKeyEventFilter::new().for_verifying_key(id.clone()),
        );
        assert!(ok_filter.matches(&ev));
        let other_id = crate::proof::VerifyingKeyId::new("halo2/ipa", "vk_other");
        let bad_filter = DataEventFilter::VerifyingKey(
            VerifyingKeyEventFilter::new().for_verifying_key(other_id),
        );
        assert!(!bad_filter.matches(&ev));
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn offline_filter_matches_platform_policy() {
        use core::str::FromStr;

        use crate::{
            account::AccountId,
            asset::AssetDefinitionId,
            events::data::offline::{
                OfflineTransferArchived, OfflineTransferEvent, OfflineTransferSettled,
            },
            offline::{AndroidIntegrityPolicy, OfflinePlatformTokenSnapshot},
        };

        let controller =
            AccountId::parse_encoded("6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw")
                .map(crate::account::ParsedAccountId::into_account_id)
                .unwrap();
        let receiver = AccountId::new(
            "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D"
                .parse()
                .expect("public key"),
        );
        let deposit_account = AccountId::new(
            "ed0120ED77765E503B45FF9C059A1C19BF1DDE82C60432B7C2D01F7FCD75F5F9F3C07C"
                .parse()
                .expect("public key"),
        );
        let asset_definition = AssetDefinitionId::from_str("xor#wonderland").unwrap();
        let platform_snapshot = OfflinePlatformTokenSnapshot {
            policy: AndroidIntegrityPolicy::PlayIntegrity.as_str().to_string(),
            attestation_jws_b64: "token".into(),
        };
        let settled = OfflineTransferEvent::Settled(OfflineTransferSettled {
            bundle_id: Hash::new([1; 32]),
            controller: controller.clone(),
            receiver: receiver.clone(),
            deposit_account: deposit_account.clone(),
            asset_definition,
            amount: Numeric::new(100, 0),
            receipt_count: 1,
            recorded_at_ms: 1_700_000_000,
            platform_snapshot: Some(platform_snapshot),
        });
        let filter = OfflineTransferEventFilter::new()
            .for_platform_policy(AndroidIntegrityPolicy::PlayIntegrity);
        assert!(filter.matches(&settled));

        let mismatch_filter = OfflineTransferEventFilter::new()
            .for_platform_policy(AndroidIntegrityPolicy::HmsSafetyDetect);
        assert!(!mismatch_filter.matches(&settled));

        let missing_snapshot = OfflineTransferEvent::Settled(OfflineTransferSettled {
            bundle_id: Hash::new([2; 32]),
            controller,
            receiver,
            deposit_account: deposit_account.clone(),
            asset_definition: AssetDefinitionId::from_str("usd#wonderland").unwrap(),
            amount: Numeric::new(25, 0),
            receipt_count: 2,
            recorded_at_ms: 1_700_000_100,
            platform_snapshot: None,
        });
        assert!(!filter.matches(&missing_snapshot));

        let archived = OfflineTransferEvent::Archived(OfflineTransferArchived {
            bundle_id: Hash::new([1; 32]),
            recorded_at_height: 10,
            archived_at_height: 20,
            archived_at_ms: 1_700_100_000,
        });
        assert!(!filter.matches(&archived));
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn proof_event_filter_matches_by_id_and_preset() {
        use crate::events::data::proof::{ProofEvent, ProofEventSet, ProofRejected, ProofVerified};
        // Build a fixed proof id
        let mut h = [0u8; 32];
        h[0] = 0xAB;
        let pid = crate::proof::ProofId {
            backend: "halo2/ipa".into(),
            proof_hash: h,
        };

        // Event: Verified with this id
        let ev_verified = DataEvent::Proof(ProofEvent::Verified(ProofVerified {
            id: pid.clone(),
            vk_ref: None,
            vk_commitment: None,
            call_hash: None,
            envelope_hash: None,
        }));
        // Event: Rejected with this id
        let ev_rejected = DataEvent::Proof(ProofEvent::Rejected(ProofRejected {
            id: pid.clone(),
            vk_ref: None,
            vk_commitment: None,
            call_hash: None,
            envelope_hash: None,
        }));

        // Filter by id, only verified
        let f_only_verified = DataEventFilter::Proof(
            ProofEventFilter::new()
                .for_proof(pid.clone())
                .for_events(ProofEventSet::only_verified()),
        );
        assert!(f_only_verified.matches(&ev_verified));
        assert!(!f_only_verified.matches(&ev_rejected));

        // Filter by id, only rejected
        let f_only_rejected = DataEventFilter::Proof(
            ProofEventFilter::new()
                .for_proof(pid.clone())
                .for_events(ProofEventSet::only_rejected()),
        );
        assert!(f_only_rejected.matches(&ev_rejected));
        assert!(!f_only_rejected.matches(&ev_verified));

        // Filter with different id should not match
        let mut h2 = [0u8; 32];
        h2[0] = 0xCD;
        let other = crate::proof::ProofId {
            backend: "halo2/ipa".into(),
            proof_hash: h2,
        };
        let f_other = DataEventFilter::Proof(
            ProofEventFilter::new()
                .for_proof(other)
                .for_events(ProofEventSet::only_verified()),
        );
        assert!(!f_other.matches(&ev_verified));
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn nft_filter_matches_nested_events() {
        let domain_id: DomainId = "genesis".parse().unwrap();
        let domain_label = domain_id.name().as_ref();
        let nft_id: NftId = format!("dragon${domain_label}").parse().unwrap();
        let other_nft_id: NftId = format!("phoenix${domain_label}").parse().unwrap();

        let nft_event = DataEvent::Domain(DomainEvent::Nft(NftEvent::Deleted(nft_id.clone())));
        let matching_filter = DataEventFilter::Nft(NftEventFilter::new().for_nft(nft_id.clone()));
        assert!(matching_filter.matches(&nft_event));

        let mismatching_filter =
            DataEventFilter::Nft(NftEventFilter::new().for_nft(other_nft_id.clone()));
        assert!(!mismatching_filter.matches(&nft_event));

        // Domain-level filter should still match the nested NFT event.
        let domain_filter =
            DataEventFilter::Domain(DomainEventFilter::new().for_domain(domain_id.clone()));
        assert!(domain_filter.matches(&nft_event));
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn runtime_upgrade_filter_matches_by_id() {
        use crate::events::data::runtime_upgrade::{RuntimeUpgradeEvent, RuntimeUpgradeProposed};

        let upgrade_id = crate::runtime::RuntimeUpgradeId([0xAB; 32]);
        let event =
            DataEvent::RuntimeUpgrade(RuntimeUpgradeEvent::Proposed(RuntimeUpgradeProposed {
                id: upgrade_id,
                abi_version: 1,
                start_height: 10,
                end_height: 20,
            }));

        let matching_filter = DataEventFilter::RuntimeUpgrade(
            RuntimeUpgradeEventFilter::new().for_runtime(upgrade_id),
        );
        assert!(matching_filter.matches(&event));

        let other_filter = DataEventFilter::RuntimeUpgrade(
            RuntimeUpgradeEventFilter::new()
                .for_runtime(crate::runtime::RuntimeUpgradeId([0xAC; 32])),
        );
        assert!(!other_filter.matches(&event));

        let event_set_filter = DataEventFilter::RuntimeUpgrade(
            RuntimeUpgradeEventFilter::new()
                .for_events(crate::events::data::runtime_upgrade::RuntimeUpgradeEventSet::all()),
        );
        assert!(event_set_filter.matches(&event));
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn space_directory_filter_matches_scope() {
        use crate::events::data::space_directory::{
            SpaceDirectoryEvent, SpaceDirectoryManifestActivated,
        };
        let dataspace = DataSpaceId::new(42);
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid:test"));
        let manifest_hash = Hash::new(b"manifest:test");
        let event = DataEvent::SpaceDirectory(SpaceDirectoryEvent::ManifestActivated(
            SpaceDirectoryManifestActivated {
                dataspace,
                uaid,
                manifest_hash,
                activation_epoch: 10,
                expiry_epoch: Some(20),
            },
        ));

        let matching = DataEventFilter::SpaceDirectory(
            SpaceDirectoryEventFilter::new().for_dataspace(dataspace),
        );
        assert!(matching.matches(&event));

        let mismatching = DataEventFilter::SpaceDirectory(
            SpaceDirectoryEventFilter::new().for_dataspace(DataSpaceId::new(7)),
        );
        assert!(!mismatching.matches(&event));
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn oracle_filter_matches_feed_id() {
        use crate::{
            events::data::oracle::OracleEvent,
            oracle::{FeedConfigVersion, FeedEvent, FeedEventOutcome},
        };

        let feed_id: crate::oracle::FeedId = "price_xor_usd".parse().unwrap();
        let event = DataEvent::Oracle(OracleEvent::FeedProcessed(
            crate::events::data::oracle::FeedEventRecord {
                event: FeedEvent {
                    feed_id: feed_id.clone(),
                    feed_config_version: FeedConfigVersion(1),
                    slot: 10,
                    outcome: FeedEventOutcome::Missing,
                },
                evidence_hashes: Vec::new(),
            },
        ));

        let matching = DataEventFilter::Oracle(OracleEventFilter::new().for_feed(feed_id.clone()));
        assert!(matching.matches(&event));

        let mismatching = DataEventFilter::Oracle(
            OracleEventFilter::new().for_feed("other_feed".parse().unwrap()),
        );
        assert!(!mismatching.matches(&event));
    }
}

#[allow(dead_code)]
mod bridge_filters_model {
    use iroha_data_model_derive::model;
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    use crate::events::data::events::HasOrigin;

    #[model]
    mod model {
        use super::*;

        /// An event filter for `BridgeEvent`
        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            getset::Getters,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct BridgeEventFilter {
            pub(super) id_matcher: Option<crate::nexus::LaneId>,
            pub(super) event_set: crate::events::data::events::bridge::BridgeEventSet,
        }
    }

    pub use model::BridgeEventFilter;

    impl BridgeEventFilter {
        /// Creates a new [`BridgeEventFilter`] accepting all bridge events.
        pub fn new() -> Self {
            Self {
                id_matcher: None,
                event_set: crate::events::data::events::bridge::BridgeEventSet::all(),
            }
        }

        /// Restrict the filter to events originating from the provided lane id.
        #[must_use]
        pub fn for_lane(mut self, lane: crate::nexus::LaneId) -> Self {
            self.id_matcher = Some(lane);
            self
        }

        /// Restrict the filter to specific bridge event kinds.
        #[must_use]
        pub fn for_events(
            mut self,
            event_set: crate::events::data::events::bridge::BridgeEventSet,
        ) -> Self {
            self.event_set = event_set;
            self
        }

        /// Check if a bridge event matches this filter.
        pub fn matches_event(
            &self,
            event: &crate::events::data::events::bridge::BridgeEvent,
        ) -> bool {
            let origin_ok = self
                .id_matcher
                .as_ref()
                .is_none_or(|lane| lane == event.origin());
            origin_ok && self.event_set.matches(event)
        }
    }

    impl Default for BridgeEventFilter {
        fn default() -> Self {
            Self::new()
        }
    }
}
