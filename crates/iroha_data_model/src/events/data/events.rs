//! Data event definitions used across the Iroha ledger.

use std::{string::String, vec::Vec};

use getset::Getters;
use iroha_data_model_derive::{EventSet, HasOrigin, model};
use iroha_primitives::{json::Json, numeric::Numeric};
#[allow(unused_imports)]
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};

pub use self::model::*;
use super::*;

macro_rules! data_event {
    ($(#[$meta:meta])* $vis:vis enum $name:ident { $($body:tt)* }) => {
        iroha_data_model_derive::model_single! {
            #[derive(
                Debug,
                Clone,
                PartialEq,
                Eq,
                PartialOrd,
                Ord,
                HasOrigin,
                EventSet,
                Decode,
                Encode,
                iroha_schema::IntoSchema,
            )]
            #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
            $(#[$meta])* $vis enum $name { $($body)* }
        }

        #[cfg(feature = "json")]
        impl_json_via_norito_bytes!($name);
    };
}

// NOTE: if adding/editing events here, make sure to update the corresponding event filter in [`super::filter`]

#[model]
mod model {
    use super::*;

    /// Generic [`MetadataChanged`] struct.
    /// Contains the changed metadata (`(key, value)` pair), either inserted or removed, which is determined by the wrapping event.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Getters)]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct MetadataChanged<Id> {
        pub target: Id,
        pub key: Name,
        pub value: Json,
    }

    /// Event
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FromVariant, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum DataEvent {
        /// Peer event
        Peer(peer::PeerEvent),
        /// Domain event
        Domain(domain::DomainEvent),
        /// Trigger event
        Trigger(trigger::TriggerEvent),
        /// Role event
        Role(role::RoleEvent),
        /// Configuration event
        Configuration(config::ConfigurationEvent),
        /// Executor event
        Executor(executor::ExecutorEvent),
        /// Zero-knowledge proof verification event
        Proof(proof::ProofEvent),
        /// Confidential asset lifecycle events
        Confidential(super::confidential::ConfidentialEvent),
        /// Verifying key registry lifecycle events
        VerifyingKey(super::verifying_keys::VerifyingKeyEvent),
        /// Runtime upgrade lifecycle events
        RuntimeUpgrade(super::runtime_upgrade::RuntimeUpgradeEvent),
        /// Smart contract registry events
        SmartContract(super::smart_contract::SmartContractEvent),
        /// Resolver attestation directory governance events
        Soradns(super::soradns::SoradnsDirectoryEvent),
        /// `SoraFS` gateway compliance events
        Sorafs(super::sorafs::SorafsGatewayEvent),
        /// Offline settlement lifecycle events
        Offline(super::offline::OfflineTransferEvent),
        /// Space Directory manifest lifecycle events
        SpaceDirectory(super::space_directory::SpaceDirectoryEvent),
        /// Oracle feed aggregation lifecycle events
        Oracle(super::oracle::OracleEvent),
        #[cfg(feature = "governance")]
        /// Governance lifecycle events
        Governance(super::governance::GovernanceEvent),
        /// Viral incentive lifecycle events
        Social(super::social::SocialEvent),
        /// Bridge event
        Bridge(bridge::BridgeEvent),
    }
}

#[cfg(feature = "json")]
impl<Id> JsonSerialize for MetadataChanged<Id>
where
    Id: JsonSerialize,
{
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        json::write_json_string("target", out);
        out.push(':');
        <Id as JsonSerialize>::json_serialize(&self.target, out);
        out.push(',');
        json::write_json_string("key", out);
        out.push(':');
        <Name as JsonSerialize>::json_serialize(&self.key, out);
        out.push(',');
        json::write_json_string("value", out);
        out.push(':');
        <Json as JsonSerialize>::json_serialize(&self.value, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl<Id> JsonDeserialize for MetadataChanged<Id>
where
    Id: JsonDeserialize,
{
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;

        let mut target: Option<Id> = None;
        let mut key: Option<Name> = None;
        let mut value: Option<Json> = None;

        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }

            let field = parser.parse_key()?;
            match field.as_str() {
                "target" => {
                    if target.is_some() {
                        return Err(json::Error::duplicate_field("target"));
                    }
                    target = Some(<Id as JsonDeserialize>::json_deserialize(parser)?);
                }
                "key" => {
                    if key.is_some() {
                        return Err(json::Error::duplicate_field("key"));
                    }
                    key = Some(<Name as JsonDeserialize>::json_deserialize(parser)?);
                }
                "value" => {
                    if value.is_some() {
                        return Err(json::Error::duplicate_field("value"));
                    }
                    value = Some(<Json as JsonDeserialize>::json_deserialize(parser)?);
                }
                other => {
                    return Err(json::Error::unknown_field(other.to_owned()));
                }
            }

            parser.skip_ws();
            if parser.try_consume_char(b',')? {
                continue;
            }
            parser.consume_char(b'}')?;
            break;
        }

        Ok(MetadataChanged {
            target: target.ok_or_else(|| json::Error::missing_field("target"))?,
            key: key.ok_or_else(|| json::Error::missing_field("key"))?,
            value: value.ok_or_else(|| json::Error::missing_field("value"))?,
        })
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(DataEvent);

pub mod confidential {
    //! Confidential asset events (shield, transfer, unshield).

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::*;

    data_event! {
        #[has_origin(origin = AssetDefinition)]
        /// Event emitted for confidential ledger operations.
        pub enum ConfidentialEvent {
            #[has_origin(shielded => &shielded.asset_definition)]
            /// A confidential asset was shielded.
            Shielded(ConfidentialShielded),
            #[has_origin(transferred => &transferred.asset_definition)]
            /// Confidential notes were transferred.
            Transferred(ConfidentialTransferred),
            #[has_origin(unshielded => &unshielded.asset_definition)]
            /// Confidential notes were unshielded into a public balance.
            Unshielded(ConfidentialUnshielded),
        }
    }

    #[model]
    mod model {
        use super::*;
        /// Event payload produced by confidential shield operations.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
        )]
        pub struct ConfidentialShielded {
            /// Asset definition whose confidential ledger was updated.
            pub asset_definition: AssetDefinitionId,
            /// Account that initiated the shield.
            pub account: AccountId,
            /// Note commitment appended to the shielded ledger.
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
            pub commitment: [u8; 32],
            /// Merkle root before the shield (if any).
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option")
            )]
            pub root_before: Option<[u8; 32]>,
            /// Merkle root after the shield.
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
            pub root_after: [u8; 32],
            /// Transaction call hash that produced the shield.
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option")
            )]
            pub call_hash: Option<[u8; 32]>,
        }

        /// Event payload produced by confidential transfer operations.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
        )]
        pub struct ConfidentialTransferred {
            /// Asset definition whose confidential ledger was updated.
            pub asset_definition: AssetDefinitionId,
            /// Nullifiers consumed by the transfer.
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::vec")
            )]
            pub nullifiers: Vec<[u8; 32]>,
            /// Output commitments appended to the ledger (sorted deterministically).
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::vec")
            )]
            pub outputs: Vec<[u8; 32]>,
            /// Merkle root before the transfer (if any).
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option")
            )]
            pub root_before: Option<[u8; 32]>,
            /// Merkle root after the transfer.
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
            pub root_after: [u8; 32],
            /// Blake2b-derived proof hash used for registry lookups.
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
            pub proof_hash: [u8; 32],
            /// Optional hash of the verification envelope (Norito payload).
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option")
            )]
            pub envelope_hash: Option<[u8; 32]>,
            /// Transaction call hash associated with the transfer.
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option")
            )]
            pub call_hash: Option<[u8; 32]>,
        }

        /// Event payload produced by confidential unshield operations.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
        )]
        pub struct ConfidentialUnshielded {
            /// Asset definition whose confidential ledger supplied the notes.
            pub asset_definition: AssetDefinitionId,
            /// Account credited with the transparent amount.
            pub account: AccountId,
            /// Public amount credited as part of the unshield.
            pub public_amount: u128,
            /// Nullifiers consumed by the unshield operation.
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::vec")
            )]
            pub nullifiers: Vec<[u8; 32]>,
            /// Optional root hint supplied by the transaction.
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option")
            )]
            pub root_hint: Option<[u8; 32]>,
            /// Blake2b-derived proof hash used for registry lookups.
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
            pub proof_hash: [u8; 32],
            /// Optional hash of the verification envelope (Norito payload).
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option")
            )]
            pub envelope_hash: Option<[u8; 32]>,
            /// Transaction call hash associated with the unshield.
            #[cfg_attr(
                feature = "json",
                norito(with = "crate::json_helpers::fixed_bytes::option")
            )]
            pub call_hash: Option<[u8; 32]>,
        }
    }

    /// Prelude exports for confidential events.
    #[allow(unused_imports)]
    pub mod prelude {
        pub use super::{
            ConfidentialEvent, ConfidentialEventSet, ConfidentialShielded, ConfidentialTransferred,
            ConfidentialUnshielded,
        };
    }

    impl ConfidentialEventSet {
        /// Matches only shield events.
        pub const fn only_shielded() -> Self {
            Self::Shielded
        }

        /// Matches only transfer events.
        pub const fn only_transferred() -> Self {
            Self::Transferred
        }

        /// Matches only unshield events.
        pub const fn only_unshielded() -> Self {
            Self::Unshielded
        }
    }
}

mod asset {
    //! This module contains `AssetEvent`, `AssetDefinitionEvent` and its impls

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::*;

    /// Metadata update associated with a specific asset instance.
    pub type AssetMetadataChanged = MetadataChanged<AssetId>;
    type AssetDefinitionMetadataChanged = MetadataChanged<AssetDefinitionId>;

    data_event! {
        #[has_origin(origin = Asset)]
        /// Event describing changes to an individual asset.
        pub enum AssetEvent {
            #[has_origin(asset => asset.id())]
            /// Asset instance was created.
            Created(Asset),
            /// Asset instance was deleted.
            Deleted(AssetId),
            #[has_origin(asset_changed => &asset_changed.asset)]
            /// Asset quantity increased.
            Added(AssetChanged),
            #[has_origin(asset_changed => &asset_changed.asset)]
            /// Asset quantity decreased.
            Removed(AssetChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was inserted or updated.
            MetadataInserted(AssetMetadataChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was removed.
            MetadataRemoved(AssetMetadataChanged),
        }
    }

    data_event! {
        #[has_origin(origin = AssetDefinition)]
        /// Event describing lifecycle of an asset definition.
        pub enum AssetDefinitionEvent {
            #[has_origin(asset_definition => asset_definition.id())]
            /// Asset definition was registered.
            Created(AssetDefinition),
            /// Asset definition was deleted.
            Deleted(AssetDefinitionId),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was inserted or updated.
            MetadataInserted(AssetDefinitionMetadataChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was removed.
            MetadataRemoved(AssetDefinitionMetadataChanged),
            /// Mintability flag toggled.
            MintabilityChanged(AssetDefinitionId),
            /// Mintability flipped from `Once` to `Not` on first mint with details.
            #[has_origin(mintability_changed => &mintability_changed.asset_definition)]
            MintabilityChangedDetailed(AssetDefinitionMintabilityChanged),
            #[has_origin(total_quantity_changed => &total_quantity_changed.asset_definition)]
            /// Total quantity value changed.
            TotalQuantityChanged(AssetDefinitionTotalQuantityChanged),
            #[has_origin(ownership_changed => &ownership_changed.asset_definition)]
            /// Owner field changed.
            OwnerChanged(AssetDefinitionOwnerChanged),
        }
    }

    #[model]
    mod model {
        use super::*;

        /// Depending on the wrapping event, [`Self`] represents the added or removed asset quantity.
        #[allow(clippy::ref_option)]
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[getset(get = "pub")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct AssetChanged {
            pub asset: AssetId,
            pub amount: Numeric,
        }

        /// [`Self`] represents updated total asset quantity.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct AssetDefinitionTotalQuantityChanged {
            pub asset_definition: AssetDefinitionId,
            pub total_amount: Numeric,
        }

        /// [`Self`] represents updated total asset quantity.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct AssetDefinitionOwnerChanged {
            /// Id of asset definition being updated
            pub asset_definition: AssetDefinitionId,
            /// Id of new owning account
            pub new_owner: AccountId,
        }

        /// Emitted together with [`AssetDefinitionEvent::MintabilityChanged`]
        /// when a limited asset definition (either `Mintable::Once` or `Mintable::Limited`)
        /// exhausts its mintability budget and flips to `Mintable::Not`.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct AssetDefinitionMintabilityChanged {
            /// Id of the asset definition that flipped to `Not`.
            pub asset_definition: AssetDefinitionId,
            /// Amount minted in the flipping transaction.
            pub minted_amount: Numeric,
            /// Account that performed the mint.
            pub authority: AccountId,
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    AssetChanged,
    AssetDefinitionTotalQuantityChanged,
    AssetDefinitionOwnerChanged,
    AssetDefinitionMintabilityChanged,
);

pub mod bridge {
    //! Bridge events

    use super::*;
    use crate::nexus::LaneId;

    data_event! {
        /// Bridge lane events
        #[has_origin(origin = LaneId)]
        pub enum BridgeEvent {
            /// Emitted when a bridge receipt is recorded
            #[has_origin(receipt => &receipt.lane)]
            Emitted(crate::bridge::BridgeReceipt),
        }
    }
}

mod nft {
    //! This module contains `NftEvent` and its impls

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::*;

    /// Metadata change captured for a specific NFT instance.
    type NftMetadataChanged = MetadataChanged<NftId>;

    data_event! {
        #[has_origin(origin = Nft)]
        /// Event describing lifecycle changes for a single NFT.
        pub enum NftEvent {
            #[has_origin(nft => nft.id())]
            /// NFT was created.
            Created(Nft),
            /// NFT was deleted.
            Deleted(NftId),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was inserted or updated.
            MetadataInserted(NftMetadataChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was removed.
            MetadataRemoved(NftMetadataChanged),
            #[has_origin(ownership_changed => &ownership_changed.nft)]
            /// NFT ownership changed.
            OwnerChanged(NftOwnerChanged),
        }
    }

    #[model]
    mod model {
        use super::*;

        /// Event indicates that owner of the [`Nft`] is changed
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct NftOwnerChanged {
            /// Id of NFT being updated
            pub nft: NftId,
            /// Id of new owning account
            pub new_owner: AccountId,
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(NftOwnerChanged);

mod peer {
    //! This module contains `PeerEvent` and its impls

    use super::*;

    data_event! {
        #[has_origin(origin = Peer)]
        /// Event emitted when peers join or leave the network view.
        pub enum PeerEvent {
            /// A peer joined the topology.
            Added(PeerId),
            /// A peer was removed from the topology.
            Removed(PeerId),
        }
    }
}

mod role {
    //! This module contains `RoleEvent` and its impls

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::*;

    data_event! {
        #[has_origin(origin = Role)]
        /// Event describing role lifecycle and permissions.
        pub enum RoleEvent {
            #[has_origin(role => role.id())]
            /// Role was created.
            Created(Role),
            /// Role was deleted.
            Deleted(RoleId),
            /// [`Permission`] were added to the role.
            #[has_origin(permission_added => &permission_added.role)]
            PermissionAdded(RolePermissionChanged),
            /// [`Permission`] were removed from the role.
            #[has_origin(permission_removed => &permission_removed.role)]
            PermissionRemoved(RolePermissionChanged),
        }
    }

    #[model]
    mod model {
        use super::*;

        /// Depending on the wrapping event, [`RolePermissionChanged`] role represents the added or removed role's permission
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct RolePermissionChanged {
            pub role: RoleId,
            // Getter derived via `getset` is skipped so the field remains opaque to FFI bindings.
            pub permission: Permission,
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(RolePermissionChanged);

impl RolePermissionChanged {
    /// Create a new [`RolePermissionChanged`] event payload.
    #[must_use]
    pub fn new(role: RoleId, permission: Permission) -> Self {
        Self { role, permission }
    }

    /// Permission that was added to or removed from the role.
    #[must_use]
    pub fn permission(&self) -> &Permission {
        &self.permission
    }
}

mod account {
    //! This module contains `AccountEvent` and its impls

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::{repo_account::RepoAccountEvent, *};

    /// Metadata change associated with a specific account.
    type AccountMetadataChanged = MetadataChanged<AccountId>;

    data_event! {
        #[has_origin(origin = Account)]
        /// Event describing changes applied to an account.
        pub enum AccountEvent {
            #[has_origin(account => account.id())]
            /// Account was created.
            Created(Account),
            /// Account was deleted.
            Deleted(AccountId),
            #[has_origin(asset_event => &asset_event.origin().account)]
            /// Nested asset event scoped to this account.
            Asset(AssetEvent),
            #[has_origin(permission_changed => &permission_changed.account)]
            /// Permission was granted to the account.
            PermissionAdded(AccountPermissionChanged),
            #[has_origin(permission_changed => &permission_changed.account)]
            /// Permission was revoked from the account.
            PermissionRemoved(AccountPermissionChanged),
            #[has_origin(role_changed => &role_changed.account)]
            /// Role was granted to the account.
            RoleGranted(AccountRoleChanged),
            #[has_origin(role_changed => &role_changed.account)]
            /// Role was revoked from the account.
            RoleRevoked(AccountRoleChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was inserted or updated.
            MetadataInserted(AccountMetadataChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was removed.
            MetadataRemoved(AccountMetadataChanged),
            #[has_origin(repo_event => repo_event.origin())]
            /// Repo agreement lifecycle event scoped to this account.
            Repo(RepoAccountEvent),
        }
    }

    #[model]
    mod model {
        use super::*;

        /// Depending on the wrapping event, [`AccountPermissionChanged`] role represents the added or removed account role
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct AccountPermissionChanged {
            pub account: AccountId,
            // Getter derived via `getset` is skipped so the field remains opaque to FFI bindings.
            pub permission: Permission,
        }

        /// Depending on the wrapping event, [`AccountRoleChanged`] represents the granted or revoked role
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct AccountRoleChanged {
            pub account: AccountId,
            pub role: RoleId,
        }
    }

    impl AccountPermissionChanged {
        /// Create a new [`AccountPermissionChanged`] event payload.
        #[must_use]
        pub fn new(account: AccountId, permission: Permission) -> Self {
            Self {
                account,
                permission,
            }
        }

        /// Permission that was added to or removed from the account.
        #[must_use]
        pub fn permission(&self) -> &Permission {
            &self.permission
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(AccountPermissionChanged, AccountRoleChanged);

mod repo_account {
    //! Repo lifecycle events scoped to individual accounts.

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::*;
    use crate::{
        account::AccountId,
        repo::{RepoAgreement, RepoAgreementId, RepoCashLeg, RepoCollateralLeg},
    };

    data_event! {
        #[has_origin(origin = Account)]
        /// Repo agreement lifecycle event emitted for a specific account perspective.
        pub enum RepoAccountEvent {
            #[has_origin(initiated => &initiated.account)]
            /// The account participated in a newly initiated repo agreement.
            Initiated(RepoAccountInitiated),
            #[has_origin(settled => &settled.account)]
            /// The account participated in a repo settlement.
            Settled(RepoAccountSettled),
            #[has_origin(margin_called => &margin_called.account)]
            /// The account received a margin call notification.
            MarginCalled(RepoAccountMarginCalled),
        }
    }

    /// Role played by the account within a repo agreement lifecycle event.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    /// Role carried by accounts participating in the repository subsystem.
    pub enum RepoAccountRole {
        /// Account initiated the agreement (borrower).
        Initiator,
        /// Account acted as counterparty (lender).
        Counterparty,
        /// Account served as the collateral custodian in a tri-party agreement.
        Custodian,
    }

    #[model]
    mod model {
        use super::*;

        /// Repo initiation payload for a particular participant.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct RepoAccountInitiated {
            /// Account receiving the event.
            pub account: AccountId,
            /// Counterparty involved in the agreement.
            pub counterparty: AccountId,
            /// Full agreement record captured on initiation.
            pub agreement: RepoAgreement,
            /// Whether the account initiated or accepted the agreement.
            pub role: RepoAccountRole,
        }

        /// Repo settlement payload for a particular participant.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct RepoAccountSettled {
            /// Account receiving the event.
            pub account: AccountId,
            /// Counterparty in the agreement.
            pub counterparty: AccountId,
            /// Identifier of the settled agreement.
            pub agreement_id: RepoAgreementId,
            /// Cash leg repaid on settlement (principal plus any accrued interest).
            pub cash_leg: RepoCashLeg,
            /// Collateral returned to this account.
            pub collateral_leg: RepoCollateralLeg,
            /// Timestamp (milliseconds since epoch) recorded for settlement.
            pub settled_timestamp_ms: u64,
            /// Whether the account was the initiator or counterparty.
            pub role: RepoAccountRole,
        }

        /// Repo margin call payload for a particular participant.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct RepoAccountMarginCalled {
            /// Account receiving the margin call event.
            pub account: AccountId,
            /// Counterparty in the agreement (borrower vs lender).
            pub counterparty: AccountId,
            /// Identifier of the agreement subject to the margin check.
            pub agreement_id: RepoAgreementId,
            /// Timestamp (milliseconds since epoch) recorded for the margin call.
            pub margin_timestamp_ms: u64,
            /// Role the account held in the agreement.
            pub role: RepoAccountRole,
        }
    }

    impl RepoAccountInitiated {
        /// Create an account-scoped initiation payload.
        #[must_use]
        pub fn new(
            account: AccountId,
            counterparty: AccountId,
            agreement: RepoAgreement,
            role: RepoAccountRole,
        ) -> Self {
            Self {
                account,
                counterparty,
                agreement,
                role,
            }
        }
    }

    impl RepoAccountSettled {
        /// Create an account-scoped settlement payload.
        #[allow(clippy::too_many_arguments)]
        #[must_use]
        pub fn new(
            account: AccountId,
            counterparty: AccountId,
            agreement_id: RepoAgreementId,
            cash_leg: RepoCashLeg,
            collateral_leg: RepoCollateralLeg,
            settled_timestamp_ms: u64,
            role: RepoAccountRole,
        ) -> Self {
            Self {
                account,
                counterparty,
                agreement_id,
                cash_leg,
                collateral_leg,
                settled_timestamp_ms,
                role,
            }
        }
    }

    impl RepoAccountMarginCalled {
        /// Create an account-scoped margin call payload.
        #[must_use]
        pub fn new(
            account: AccountId,
            counterparty: AccountId,
            agreement_id: RepoAgreementId,
            margin_timestamp_ms: u64,
            role: RepoAccountRole,
        ) -> Self {
            Self {
                account,
                counterparty,
                agreement_id,
                margin_timestamp_ms,
                role,
            }
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    repo_account::RepoAccountInitiated,
    repo_account::RepoAccountSettled
);

mod domain {
    //! This module contains `DomainEvent` and its impls

    pub use self::model::*;
    use super::*;

    /// Metadata change associated with a specific domain.
    type DomainMetadataChanged = MetadataChanged<DomainId>;

    data_event! {
        #[has_origin(origin = Domain)]
        /// Event describing changes within a domain.
        pub enum DomainEvent {
            #[has_origin(domain => domain.id())]
            /// Domain was created.
            Created(Domain),
            /// Domain was deleted.
            Deleted(DomainId),
            #[has_origin(asset_definition_event => &asset_definition_event.origin().domain)]
            /// Asset-definition event occurred in the domain scope.
            AssetDefinition(AssetDefinitionEvent),
            #[has_origin(nft_event => &nft_event.origin().domain)]
            /// NFT event occurred in the domain scope.
            Nft(NftEvent),
            #[has_origin(account_event => &account_event.origin().domain)]
            /// Account event occurred in the domain scope.
            Account(AccountEvent),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was inserted or updated.
            MetadataInserted(DomainMetadataChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was removed.
            MetadataRemoved(DomainMetadataChanged),
            #[has_origin(owner_changed => &owner_changed.domain)]
            /// Domain owner changed.
            OwnerChanged(DomainOwnerChanged),
            #[has_origin(summary => &summary.call.domain_id)]
            /// Aggregated Kaigi roster metrics were recorded.
            KaigiRosterSummary(KaigiRosterSummary),
            #[has_origin(summary => &summary.relay.domain)]
            /// Kaigi relay registration summary emitted.
            KaigiRelayRegistered(model::KaigiRelayRegistrationSummary),
            #[has_origin(summary => &summary.call.domain_id)]
            /// Kaigi relay manifest updated.
            KaigiRelayManifestUpdated(KaigiRelayManifestSummary),
            #[has_origin(summary => &summary.call.domain_id)]
            /// Kaigi usage metrics recorded.
            KaigiUsageSummary(KaigiUsageSummary),
            #[has_origin(summary => &summary.call.domain_id)]
            /// Kaigi relay health status changed.
            KaigiRelayHealthUpdated(KaigiRelayHealthSummary),
            #[has_origin(ticket_ready => &ticket_ready.domain)]
            /// Privacy streaming ticket became available.
            StreamingTicketReady(StreamingTicketReady),
            #[has_origin(ticket_revoked => &ticket_revoked.domain)]
            /// Privacy streaming ticket was revoked.
            StreamingTicketRevoked(StreamingTicketRevoked),
        }
    }

    #[model]
    mod model {
        use iroha_crypto::Hash;
        use norito::streaming::{
            Multiaddr, PrivacyCapabilities, PrivacyRelay, PrivacyRoute, SoranetAccessKind,
            SoranetChannelId, SoranetRoute, SoranetStreamTag,
        };

        use super::*;
        use crate::{
            DataSpaceId, LaneId,
            account::AccountId,
            kaigi::{KaigiId, KaigiPrivacyMode, KaigiRelayHealthStatus},
            soranet::ticket::TicketEnvelopeV1,
        };

        /// Event indicate that owner of the [`Domain`] is changed
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct DomainOwnerChanged {
            pub domain: DomainId,
            pub new_owner: AccountId,
        }

        /// Aggregated Kaigi roster counts without exposing individual identities.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct KaigiRosterSummary {
            /// Call identifier.
            pub call: KaigiId,
            /// Privacy mode currently applied to the session.
            pub privacy_mode: KaigiPrivacyMode,
            /// Number of visible participants.
            pub participant_count: u32,
            /// Number of registered roster commitments.
            pub commitment_count: u32,
            /// Total nullifiers logged for the session.
            pub nullifier_count: u32,
            /// Current roster Merkle root (only populated in privacy mode).
            pub roster_root: Option<Hash>,
        }

        impl KaigiRosterSummary {
            /// Construct a new roster summary payload.
            #[must_use]
            pub fn new(
                call: KaigiId,
                privacy_mode: KaigiPrivacyMode,
                participant_count: u32,
                commitment_count: u32,
                nullifier_count: u32,
                roster_root: Option<Hash>,
            ) -> Self {
                Self {
                    call,
                    privacy_mode,
                    participant_count,
                    commitment_count,
                    nullifier_count,
                    roster_root,
                }
            }
        }

        /// Registration snapshot for a Kaigi relay.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct KaigiRelayRegistrationSummary {
            /// Relay identifier.
            pub relay: AccountId,
            /// Relay bandwidth class advertised during registration.
            pub bandwidth_class: u8,
            /// Fingerprint of the published HPKE public key.
            pub hpke_fingerprint: Hash,
        }

        impl KaigiRelayRegistrationSummary {
            /// Construct a new relay registration summary payload.
            #[must_use]
            pub fn new(relay: AccountId, bandwidth_class: u8, hpke_fingerprint: Hash) -> Self {
                Self {
                    relay,
                    bandwidth_class,
                    hpke_fingerprint,
                }
            }
        }

        /// Snapshot describing the active relay manifest.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct KaigiRelayManifestSummary {
            /// Call identifier.
            pub call: KaigiId,
            /// Number of hops advertised in the manifest.
            pub hop_count: u32,
            /// Manifest expiry timestamp in milliseconds.
            pub expiry_ms: u64,
        }

        impl KaigiRelayManifestSummary {
            /// Construct a new relay manifest summary payload.
            #[must_use]
            pub fn new(call: KaigiId, hop_count: u32, expiry_ms: u64) -> Self {
                Self {
                    call,
                    hop_count,
                    expiry_ms,
                }
            }
        }

        /// Health update emitted when a relay status changes.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct KaigiRelayHealthSummary {
            /// Call identifier where the health update originated.
            pub call: KaigiId,
            /// Relay account being reported.
            pub relay: AccountId,
            /// Observed health status.
            pub status: KaigiRelayHealthStatus,
            /// Timestamp (milliseconds since epoch) when the report was recorded.
            pub reported_at_ms: u64,
        }

        impl KaigiRelayHealthSummary {
            /// Construct a new relay health summary payload.
            #[must_use]
            pub fn new(
                call: KaigiId,
                relay: AccountId,
                status: KaigiRelayHealthStatus,
                reported_at_ms: u64,
            ) -> Self {
                Self {
                    call,
                    relay,
                    status,
                    reported_at_ms,
                }
            }
        }

        /// Aggregated usage totals for a Kaigi session.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct KaigiUsageSummary {
            /// Call identifier.
            pub call: KaigiId,
            /// Total duration recorded (milliseconds).
            pub total_duration_ms: u64,
            /// Total billed gas across all segments.
            pub total_billed_gas: u64,
            /// Number of segments recorded.
            pub segments_recorded: u32,
        }

        impl KaigiUsageSummary {
            /// Construct a new usage summary payload.
            #[must_use]
            pub fn new(
                call: KaigiId,
                total_duration_ms: u64,
                total_billed_gas: u64,
                segments_recorded: u32,
            ) -> Self {
                Self {
                    call,
                    total_duration_ms,
                    total_billed_gas,
                    segments_recorded,
                }
            }
        }

        /// Relay descriptor emitted alongside streaming ticket events.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct StreamingPrivacyRelay {
            pub relay_id: [u8; 32],
            pub endpoint: Multiaddr,
            pub key_fingerprint: [u8; 32],
            pub capabilities_bits: u32,
        }

        impl StreamingPrivacyRelay {
            /// Construct a new relay descriptor.
            #[must_use]
            pub fn new(
                relay_id: [u8; 32],
                endpoint: Multiaddr,
                key_fingerprint: [u8; 32],
                capabilities_bits: u32,
            ) -> Self {
                Self {
                    relay_id,
                    endpoint,
                    key_fingerprint,
                    capabilities_bits,
                }
            }
        }

        impl From<PrivacyRelay> for StreamingPrivacyRelay {
            fn from(relay: PrivacyRelay) -> Self {
                Self {
                    relay_id: relay.relay_id,
                    endpoint: relay.endpoint,
                    key_fingerprint: relay.key_fingerprint,
                    capabilities_bits: relay.capabilities.bits(),
                }
            }
        }

        impl From<&StreamingPrivacyRelay> for PrivacyRelay {
            fn from(relay: &StreamingPrivacyRelay) -> Self {
                PrivacyRelay {
                    relay_id: relay.relay_id,
                    endpoint: relay.endpoint.clone(),
                    key_fingerprint: relay.key_fingerprint,
                    capabilities: PrivacyCapabilities::from_bits(relay.capabilities_bits),
                }
            }
        }

        /// Access posture advertised by `SoraNet` exit relays.
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(
            feature = "json",
            norito(transparent, reuse_archived, decode_from_slice)
        )]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived, decode_from_slice))]
        pub enum StreamingSoranetAccessKind {
            ReadOnly,
            Authenticated,
        }

        impl From<SoranetAccessKind> for StreamingSoranetAccessKind {
            fn from(kind: SoranetAccessKind) -> Self {
                match kind {
                    SoranetAccessKind::ReadOnly => Self::ReadOnly,
                    SoranetAccessKind::Authenticated => Self::Authenticated,
                }
            }
        }

        impl From<StreamingSoranetAccessKind> for SoranetAccessKind {
            fn from(kind: StreamingSoranetAccessKind) -> Self {
                match kind {
                    StreamingSoranetAccessKind::ReadOnly => Self::ReadOnly,
                    StreamingSoranetAccessKind::Authenticated => Self::Authenticated,
                }
            }
        }

        /// Stream tags advertised by `SoraNet` relays for exit bridges.
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Default,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(
            feature = "json",
            norito(transparent, reuse_archived, decode_from_slice)
        )]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived, decode_from_slice))]
        pub enum StreamingSoranetStreamTag {
            #[default]
            NoritoStream,
            Kaigi,
        }

        impl From<SoranetStreamTag> for StreamingSoranetStreamTag {
            fn from(tag: SoranetStreamTag) -> Self {
                match tag {
                    SoranetStreamTag::NoritoStream => Self::NoritoStream,
                    SoranetStreamTag::Kaigi => Self::Kaigi,
                }
            }
        }

        impl From<StreamingSoranetStreamTag> for SoranetStreamTag {
            fn from(tag: StreamingSoranetStreamTag) -> Self {
                match tag {
                    StreamingSoranetStreamTag::NoritoStream => Self::NoritoStream,
                    StreamingSoranetStreamTag::Kaigi => Self::Kaigi,
                }
            }
        }

        /// Privacy route metadata for `SoraNet` transport.
        #[allow(clippy::ref_option)]
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[getset(get = "pub")]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(reuse_archived, decode_from_slice))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived, decode_from_slice))]
        pub struct StreamingSoranetRoute {
            pub channel_id: [u8; 32],
            pub exit_multiaddr: Multiaddr,
            #[getset(skip)]
            pub padding_budget_ms: Option<u16>,
            pub access_kind: StreamingSoranetAccessKind,
            #[getset(skip)]
            pub stream_tag: StreamingSoranetStreamTag,
        }

        impl StreamingSoranetRoute {
            /// Construct a new descriptor from raw fields.
            #[must_use]
            pub fn new(
                channel_id: [u8; 32],
                exit_multiaddr: Multiaddr,
                padding_budget_ms: Option<u16>,
                access_kind: StreamingSoranetAccessKind,
                stream_tag: StreamingSoranetStreamTag,
            ) -> Self {
                Self {
                    channel_id,
                    exit_multiaddr,
                    padding_budget_ms,
                    access_kind,
                    stream_tag,
                }
            }

            /// Returns optional padding budget (milliseconds).
            #[must_use]
            pub fn padding_budget_ms(&self) -> Option<u16> {
                self.padding_budget_ms
            }

            /// Replace the padding budget (milliseconds).
            pub fn set_padding_budget_ms(&mut self, value: Option<u16>) {
                self.padding_budget_ms = value;
            }

            /// Replace the exit relay multiaddr.
            pub fn set_exit_multiaddr(&mut self, exit: Multiaddr) {
                self.exit_multiaddr = exit;
            }

            /// Replace the advertised access posture.
            pub fn set_access_kind(&mut self, access: StreamingSoranetAccessKind) {
                self.access_kind = access;
            }

            /// Replace the advertised stream tag.
            pub fn set_stream_tag(&mut self, tag: StreamingSoranetStreamTag) {
                self.stream_tag = tag;
            }

            /// Returns the stream tag advertised by the exit.
            #[must_use]
            pub fn stream_tag(&self) -> StreamingSoranetStreamTag {
                self.stream_tag
            }
        }

        impl From<SoranetRoute> for StreamingSoranetRoute {
            fn from(route: SoranetRoute) -> Self {
                Self {
                    channel_id: <[u8; 32]>::from(route.channel_id),
                    exit_multiaddr: route.exit_multiaddr,
                    padding_budget_ms: route.padding_budget_ms,
                    access_kind: StreamingSoranetAccessKind::from(route.access_kind),
                    stream_tag: StreamingSoranetStreamTag::from(route.stream_tag),
                }
            }
        }

        impl From<&StreamingSoranetRoute> for SoranetRoute {
            fn from(route: &StreamingSoranetRoute) -> Self {
                Self {
                    channel_id: SoranetChannelId::new(route.channel_id),
                    exit_multiaddr: route.exit_multiaddr.clone(),
                    padding_budget_ms: route.padding_budget_ms,
                    access_kind: SoranetAccessKind::from(route.access_kind),
                    stream_tag: SoranetStreamTag::from(route.stream_tag),
                }
            }
        }

        /// Privacy route description mirrored from the manifest schema.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct StreamingPrivacyRoute {
            pub route_id: [u8; 32],
            pub entry: StreamingPrivacyRelay,
            pub exit: StreamingPrivacyRelay,
            pub ticket_entry: Vec<u8>,
            pub ticket_exit: Vec<u8>,
            pub expiry_segment: u64,
            #[getset(skip)]
            pub soranet: Option<StreamingSoranetRoute>,
            #[getset(skip)]
            pub ticket: Option<TicketEnvelopeV1>,
        }

        impl StreamingPrivacyRoute {
            /// Construct a new relay path descriptor (no ticket attached).
            #[must_use]
            pub fn new(
                route_id: [u8; 32],
                entry: StreamingPrivacyRelay,
                exit: StreamingPrivacyRelay,
                ticket_entry: Vec<u8>,
                ticket_exit: Vec<u8>,
                expiry_segment: u64,
            ) -> Self {
                Self {
                    route_id,
                    entry,
                    exit,
                    ticket_entry,
                    ticket_exit,
                    expiry_segment,
                    soranet: None,
                    ticket: None,
                }
            }

            /// Attach a privacy ticket envelope to the route.
            #[must_use]
            pub fn with_ticket(mut self, ticket: TicketEnvelopeV1) -> Self {
                self.ticket = Some(ticket);
                self
            }

            /// Attach `SoraNet` metadata to the route.
            #[must_use]
            pub fn with_soranet(mut self, soranet: StreamingSoranetRoute) -> Self {
                self.soranet = Some(soranet);
                self
            }

            /// Returns the attached ticket envelope, if present.
            #[must_use]
            pub fn ticket_envelope(&self) -> Option<&TicketEnvelopeV1> {
                self.ticket.as_ref()
            }

            /// Returns the attached `SoraNet` metadata, if present.
            #[must_use]
            pub fn soranet(&self) -> Option<&StreamingSoranetRoute> {
                self.soranet.as_ref()
            }

            /// Replace the attached `SoraNet` metadata.
            pub fn set_soranet(&mut self, value: Option<StreamingSoranetRoute>) {
                self.soranet = value;
            }

            /// Replace the attached ticket envelope.
            pub fn set_ticket(&mut self, value: Option<TicketEnvelopeV1>) {
                self.ticket = value;
            }

            /// Construct a streaming route from a Norito privacy route and optional ticket.
            #[must_use]
            pub fn from_parts(route: PrivacyRoute, ticket: Option<TicketEnvelopeV1>) -> Self {
                Self::from(route).with_optional_ticket(ticket)
            }

            /// Split the route into the Norito representation and an optional ticket envelope.
            #[must_use]
            pub fn into_parts(self) -> (PrivacyRoute, Option<TicketEnvelopeV1>) {
                let StreamingPrivacyRoute {
                    route_id,
                    entry,
                    exit,
                    ticket_entry,
                    ticket_exit,
                    expiry_segment,
                    soranet,
                    ticket,
                } = self;

                let privacy_route = PrivacyRoute {
                    route_id,
                    entry: PrivacyRelay::from(&entry),
                    exit: PrivacyRelay::from(&exit),
                    ticket_entry,
                    ticket_exit,
                    expiry_segment,
                    soranet: soranet.as_ref().map(SoranetRoute::from),
                };

                (privacy_route, ticket)
            }

            fn with_optional_ticket(mut self, ticket: Option<TicketEnvelopeV1>) -> Self {
                self.ticket = ticket;
                self
            }
        }

        impl From<PrivacyRoute> for StreamingPrivacyRoute {
            fn from(route: PrivacyRoute) -> Self {
                Self {
                    route_id: route.route_id,
                    entry: route.entry.into(),
                    exit: route.exit.into(),
                    ticket_entry: route.ticket_entry,
                    ticket_exit: route.ticket_exit,
                    expiry_segment: route.expiry_segment,
                    soranet: route.soranet.map(StreamingSoranetRoute::from),
                    ticket: None,
                }
            }
        }

        impl From<&StreamingPrivacyRoute> for PrivacyRoute {
            fn from(route: &StreamingPrivacyRoute) -> Self {
                PrivacyRoute {
                    route_id: route.route_id,
                    entry: PrivacyRelay::from(&route.entry),
                    exit: PrivacyRelay::from(&route.exit),
                    ticket_entry: route.ticket_entry.clone(),
                    ticket_exit: route.ticket_exit.clone(),
                    expiry_segment: route.expiry_segment,
                    soranet: route.soranet.as_ref().map(SoranetRoute::from),
                }
            }
        }

        /// Association between a privacy route and its provisioning window.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct StreamingRouteBinding {
            /// Route descriptor (entry/exit relays, HPKE tokens, expiry segment).
            pub route: StreamingPrivacyRoute,
            /// First segment (inclusive) covered by the provisioning window.
            pub valid_from_segment: u64,
            /// Last segment (inclusive) covered by the provisioning window.
            pub valid_until_segment: u64,
            /// Whether the exit relay has acknowledged provisioning.
            pub acknowledged: bool,
        }

        impl StreamingRouteBinding {
            /// Construct a new route binding descriptor.
            #[must_use]
            pub fn new(
                route: StreamingPrivacyRoute,
                valid_from_segment: u64,
                valid_until_segment: u64,
                acknowledged: bool,
            ) -> Self {
                Self {
                    route,
                    valid_from_segment,
                    valid_until_segment,
                    acknowledged,
                }
            }

            /// Convert the route descriptor into the Norito representation.
            #[must_use]
            pub fn as_privacy_route(&self) -> PrivacyRoute {
                PrivacyRoute::from(&self.route)
            }
        }

        /// Optional policy constraints embedded in streaming capability tickets.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[cfg_attr(feature = "json", norito(reuse_archived, decode_from_slice))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived, decode_from_slice))]
        #[getset(get = "pub")]
        pub struct StreamingTicketPolicy {
            /// Maximum number of relays allowed to serve this ticket concurrently.
            pub max_relays: u16,
            /// Geographic regions permitted to serve the ticket (ISO-style codes).
            pub allowed_regions: Vec<String>,
            /// Optional bandwidth ceiling in kilobits per second.
            #[getset(skip)]
            pub max_bandwidth_kbps: Option<u32>,
        }

        impl StreamingTicketPolicy {
            /// Construct a new streaming ticket policy descriptor.
            #[must_use]
            pub fn new(
                max_relays: u16,
                allowed_regions: Vec<String>,
                max_bandwidth_kbps: Option<u32>,
            ) -> Self {
                Self {
                    max_relays,
                    allowed_regions,
                    max_bandwidth_kbps,
                }
            }

            /// Returns the optional bandwidth ceiling in kilobits per second.
            #[must_use]
            pub fn max_bandwidth_kbps(&self) -> Option<u32> {
                self.max_bandwidth_kbps
            }
        }

        /// Capability bitfield advertised by streaming tickets.
        #[derive(
            Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(
            any(feature = "ffi_export", feature = "ffi_import"),
            ffi_type(unsafe {robust})
        )]
        #[cfg_attr(feature = "json", norito(transparent, reuse_archived))]
        #[cfg_attr(not(feature = "json"), norito(reuse_archived))]
        pub struct StreamingTicketCapabilities(u32);

        impl StreamingTicketCapabilities {
            /// Capability flag allowing live streaming access.
            pub const LIVE: u32 = 1 << 0;
            /// Capability flag allowing video-on-demand playback.
            pub const VOD: u32 = 1 << 1;
            /// Capability flag unlocking premium rendering profiles.
            pub const PREMIUM_PROFILE: u32 = 1 << 2;
            /// Capability flag enabling HDR ladders.
            pub const HDR: u32 = 1 << 3;
            /// Capability flag enabling spatial audio playback.
            pub const SPATIAL_AUDIO: u32 = 1 << 4;

            /// Construct capabilities from raw bits.
            #[must_use]
            pub const fn from_bits(bits: u32) -> Self {
                Self(bits)
            }

            /// Expose the underlying bit representation.
            #[must_use]
            pub const fn bits(self) -> u32 {
                self.0
            }

            /// Check whether all bits in `mask` are present.
            #[must_use]
            pub const fn contains(self, mask: u32) -> bool {
                (self.0 & mask) == mask
            }

            /// Return a new capability set with `mask` inserted.
            #[must_use]
            pub const fn insert(self, mask: u32) -> Self {
                Self(self.0 | mask)
            }

            /// Return a new capability set with `mask` removed.
            #[must_use]
            pub const fn remove(self, mask: u32) -> Self {
                Self(self.0 & !mask)
            }
        }

        impl<'a> norito::core::DecodeFromSlice<'a> for StreamingTicketCapabilities {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let (bits, used) =
                    <u32 as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
                Ok((StreamingTicketCapabilities::from_bits(bits), used))
            }
        }

        /// Streaming capability ticket metadata emitted with readiness events.
        #[allow(clippy::ref_option)]
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[norito(reuse_archived, decode_from_slice)]
        #[getset(get = "pub")]
        pub struct StreamingTicketRecord {
            /// Capability ticket identifier.
            #[getset(skip)]
            pub ticket_id: Hash,
            /// Account that owns the ticket.
            pub owner: AccountId,
            /// Data space the ticket belongs to.
            pub dsid: DataSpaceId,
            /// Execution lane scoped by the ticket.
            pub lane_id: LaneId,
            /// Settlement bucket for prepaid fan-out.
            pub settlement_bucket: u64,
            /// Slot number when the ticket becomes valid.
            pub start_slot: u64,
            /// Slot number when the ticket expires.
            pub expire_slot: u64,
            /// Prepaid traffic entitlement units.
            pub prepaid_teu: u128,
            /// Entitlement units consumed per chunk.
            pub chunk_teu: u32,
            /// Relay fan-out quota.
            pub fanout_quota: u16,
            /// Commitment to the access key.
            pub key_commitment: Hash,
            /// Ticket nonce for uniqueness.
            pub nonce: u64,
            /// Contract-level signature authorising the ticket.
            pub contract_signature: [u8; 64],
            /// Zero-knowledge commitment bound to the viewer.
            pub commitment: Hash,
            /// Nullifier preventing replay.
            #[getset(skip)]
            pub nullifier: Hash,
            /// Identifier of the verifier entry for the proof.
            pub proof_id: [u8; 32],
            /// Timestamp when the ticket was issued.
            pub issued_at: u64,
            /// Timestamp when the ticket expires.
            pub expires_at: u64,
            /// Optional policy constraints.
            #[getset(skip)]
            pub policy: Option<StreamingTicketPolicy>,
            /// Playback capabilities granted by the ticket.
            pub capabilities: StreamingTicketCapabilities,
        }

        impl StreamingTicketRecord {
            /// Construct a new ticket metadata record.
            #[allow(clippy::too_many_arguments)]
            #[must_use]
            pub fn new(
                ticket_id: Hash,
                owner: AccountId,
                dsid: DataSpaceId,
                lane_id: LaneId,
                settlement_bucket: u64,
                start_slot: u64,
                expire_slot: u64,
                prepaid_teu: u128,
                chunk_teu: u32,
                fanout_quota: u16,
                key_commitment: Hash,
                nonce: u64,
                contract_signature: [u8; 64],
                commitment: Hash,
                nullifier: Hash,
                proof_id: [u8; 32],
                issued_at: u64,
                expires_at: u64,
                policy: Option<StreamingTicketPolicy>,
                capabilities: StreamingTicketCapabilities,
            ) -> Self {
                Self {
                    ticket_id,
                    owner,
                    dsid,
                    lane_id,
                    settlement_bucket,
                    start_slot,
                    expire_slot,
                    prepaid_teu,
                    chunk_teu,
                    fanout_quota,
                    key_commitment,
                    nonce,
                    contract_signature,
                    commitment,
                    nullifier,
                    proof_id,
                    issued_at,
                    expires_at,
                    policy,
                    capabilities,
                }
            }

            /// Hash of the nullifier associated with this ticket.
            #[must_use]
            pub fn nullifier(&self) -> &Hash {
                &self.nullifier
            }

            /// Capability ticket identifier.
            #[must_use]
            pub fn ticket_id(&self) -> &Hash {
                &self.ticket_id
            }
        }

        /// Event announcing that a streaming capability ticket is ready for use.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct StreamingTicketReady {
            /// Domain identifier the stream belongs to.
            pub domain: DomainId,
            /// Stream identifier the ticket applies to.
            pub stream_id: Hash,
            /// Capability ticket metadata.
            pub ticket: StreamingTicketRecord,
            /// Provisioned privacy routes bundled with the ticket.
            pub routes: Vec<StreamingRouteBinding>,
        }

        impl StreamingTicketReady {
            /// Construct a new ticket readiness event payload.
            #[must_use]
            pub fn new(
                domain: DomainId,
                stream_id: Hash,
                ticket: StreamingTicketRecord,
                routes: Vec<StreamingRouteBinding>,
            ) -> Self {
                Self {
                    domain,
                    stream_id,
                    ticket,
                    routes,
                }
            }

            /// Convenience accessor returning the ticket identifier.
            #[must_use]
            pub fn ticket_id(&self) -> &Hash {
                self.ticket.ticket_id()
            }
        }

        /// Event indicating that a streaming capability ticket is no longer valid.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[getset(get = "pub")]
        pub struct StreamingTicketRevoked {
            /// Domain identifier the stream belongs to.
            pub domain: DomainId,
            /// Stream identifier the ticket applied to.
            pub stream_id: Hash,
            /// Capability ticket identifier.
            #[getset(skip)]
            pub ticket_id: Hash,
            /// Nullifier associated with the revoked ticket.
            pub nullifier: Hash,
            /// Reason code for revocation.
            pub reason_code: u16,
            /// Signature authorising the revocation.
            pub revocation_signature: [u8; 64],
        }

        impl StreamingTicketRevoked {
            /// Construct a new ticket revocation payload.
            #[must_use]
            pub fn new(
                domain: DomainId,
                stream_id: Hash,
                ticket_id: Hash,
                nullifier: Hash,
                reason_code: u16,
                revocation_signature: [u8; 64],
            ) -> Self {
                Self {
                    domain,
                    stream_id,
                    ticket_id,
                    nullifier,
                    reason_code,
                    revocation_signature,
                }
            }

            /// Returns the identifier of the revoked ticket.
            #[must_use]
            pub fn ticket_id(&self) -> &Hash {
                &self.ticket_id
            }
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    DomainOwnerChanged,
    domain::KaigiRosterSummary,
    domain::KaigiRelayManifestSummary,
    domain::KaigiRelayHealthSummary,
    domain::KaigiUsageSummary,
    domain::StreamingSoranetRoute,
    domain::StreamingPrivacyRelay,
    domain::StreamingPrivacyRoute,
    domain::StreamingRouteBinding,
    domain::StreamingTicketReady,
    domain::StreamingTicketRevoked
);

mod trigger {
    //! This module contains `TriggerEvent` and its impls

    use iroha_data_model_derive::model;

    pub use self::model::*;
    use super::*;

    /// Metadata change associated with a trigger instance.
    type TriggerMetadataChanged = MetadataChanged<TriggerId>;

    data_event! {
        #[has_origin(origin = Trigger)]
        /// Event describing trigger lifecycle updates.
        pub enum TriggerEvent {
            /// Trigger was created.
            Created(TriggerId),
            /// Trigger was deleted.
            Deleted(TriggerId),
            #[has_origin(number_of_executions_changed => &number_of_executions_changed.trigger)]
            /// Trigger execution window was extended.
            Extended(TriggerNumberOfExecutionsChanged),
            #[has_origin(number_of_executions_changed => &number_of_executions_changed.trigger)]
            /// Trigger execution window was shortened.
            Shortened(TriggerNumberOfExecutionsChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was inserted or updated.
            MetadataInserted(TriggerMetadataChanged),
            #[has_origin(metadata_changed => &metadata_changed.target)]
            /// Metadata entry was removed.
            MetadataRemoved(TriggerMetadataChanged),
        }
    }

    #[model]
    mod model {
        use super::*;

        /// Depending on the wrapping event, [`Self`] represents the increased or decreased number of event executions.
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct TriggerNumberOfExecutionsChanged {
            pub trigger: TriggerId,
            pub by: u32,
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(TriggerNumberOfExecutionsChanged);

mod config {
    pub use self::model::*;
    use super::*;
    use crate::parameter::Parameter;

    #[model]
    mod model {
        use super::*;

        /// Changed parameter event
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        pub struct ParameterChanged {
            /// Previous value for the parameter
            pub old_value: Parameter,
            /// Next value for the parameter
            pub new_value: Parameter,
        }

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            EventSet,
            FromVariant,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        /// Event emitted when a configuration parameter changes.
        pub enum ConfigurationEvent {
            /// Configuration parameter value changed.
            Changed(ParameterChanged),
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(ParameterChanged, ConfigurationEvent);

mod executor {
    use iroha_data_model_derive::model;

    pub use self::model::*;
    // Keep super-module imports available for generated code paths.
    #[allow(unused)]
    use super::*;

    #[model]
    mod model {

        use iroha_data_model_derive::EventSet;

        // Keep super-module imports available for generated code paths.
        #[allow(unused)]
        use super::*;
        use crate::executor::ExecutorDataModel;

        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Decode,
            Encode,
            iroha_schema::IntoSchema,
            EventSet,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
        #[cfg_attr(feature = "json", norito(untagged))] // Norito equivalent of untagged
        #[repr(transparent)]
        /// Event emitted when the executor data model is upgraded.
        pub enum ExecutorEvent {
            /// Executor data model was upgraded.
            Upgraded(ExecutorUpgrade),
        }

        /// Information about the updated executor data model.
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, Getters,
        )]
        #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
        #[repr(transparent)]
        #[getset(get = "pub")]
        pub struct ExecutorUpgrade {
            /// Updated data model
            pub new_data_model: ExecutorDataModel,
        }
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(ExecutorEvent, ExecutorUpgrade);

/// Trait for events originating from [`HasOrigin::Origin`].
pub trait HasOrigin {
    /// Type of the origin.
    type Origin: Identifiable;
    /// Identification of the origin.
    fn origin(&self) -> &<Self::Origin as Identifiable>::Id;
}

impl From<AccountEvent> for DataEvent {
    fn from(value: AccountEvent) -> Self {
        DomainEvent::Account(value).into()
    }
}

impl From<AssetDefinitionEvent> for DataEvent {
    fn from(value: AssetDefinitionEvent) -> Self {
        DomainEvent::AssetDefinition(value).into()
    }
}

impl From<AssetEvent> for DataEvent {
    fn from(value: AssetEvent) -> Self {
        AccountEvent::Asset(value).into()
    }
}

impl From<NftEvent> for DataEvent {
    fn from(value: NftEvent) -> Self {
        DomainEvent::Nft(value).into()
    }
}

impl DataEvent {
    /// Return the domain id of [`DataEvent`]
    pub fn domain(&self) -> Option<&DomainId> {
        match self {
            Self::Domain(event) => Some(event.origin()),
            #[cfg(feature = "governance")]
            Self::Governance(_) => None,
            _ => None,
        }
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::MetadataChanged;
    use crate::domain::DomainId;
    use iroha_primitives::json::Json;

    #[test]
    fn metadata_changed_json_roundtrip() {
        let changed = MetadataChanged {
            target: "default".parse().expect("valid domain"),
            key: "metadata_key".parse().expect("valid name"),
            value: Json::from("metadata_value"),
        };
        let json = norito::json::to_json(&changed).expect("serialize");
        let decoded: MetadataChanged<DomainId> =
            norito::json::from_str(&json).expect("deserialize");
        assert_eq!(changed, decoded);
    }
}

#[allow(unused_imports)]
pub mod prelude {
    pub use super::{
        DataEvent, HasOrigin, MetadataChanged,
        account::{AccountEvent, AccountEventSet, AccountPermissionChanged, AccountRoleChanged},
        asset::{
            AssetChanged, AssetDefinitionEvent, AssetDefinitionEventSet,
            AssetDefinitionMintabilityChanged, AssetDefinitionOwnerChanged,
            AssetDefinitionTotalQuantityChanged, AssetEvent, AssetEventSet, AssetMetadataChanged,
        },
        bridge::{BridgeEvent, BridgeEventSet},
        confidential::{
            ConfidentialEvent, ConfidentialEventSet, ConfidentialShielded, ConfidentialTransferred,
            ConfidentialUnshielded,
        },
        config::{ConfigurationEvent, ConfigurationEventSet, ParameterChanged},
        domain::{
            DomainEvent, DomainEventSet, DomainOwnerChanged, KaigiRelayHealthSummary,
            KaigiRelayManifestSummary, KaigiRelayRegistrationSummary, KaigiRosterSummary,
            KaigiUsageSummary, StreamingPrivacyRelay, StreamingPrivacyRoute, StreamingRouteBinding,
            StreamingSoranetAccessKind, StreamingSoranetRoute, StreamingSoranetStreamTag,
            StreamingTicketCapabilities, StreamingTicketPolicy, StreamingTicketReady,
            StreamingTicketRecord, StreamingTicketRevoked,
        },
        executor::{ExecutorEvent, ExecutorEventSet, ExecutorUpgrade},
        nft::{NftEvent, NftEventSet, NftOwnerChanged},
        peer::{PeerEvent, PeerEventSet},
        repo_account::{
            RepoAccountEvent, RepoAccountEventSet, RepoAccountInitiated, RepoAccountMarginCalled,
            RepoAccountRole, RepoAccountSettled,
        },
        role::{RoleEvent, RoleEventSet, RolePermissionChanged},
        trigger::{TriggerEvent, TriggerEventSet, TriggerNumberOfExecutionsChanged},
    };
    pub use crate::{DataSpaceId, LaneId};
}
