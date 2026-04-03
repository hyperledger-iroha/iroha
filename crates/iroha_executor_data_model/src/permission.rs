//! Definition of Iroha default permission tokens
#![allow(clippy::missing_errors_doc)]

use std::{format, string::String, vec::Vec};

use iroha_data_model::prelude::*;
pub use iroha_executor_data_model_derive::Permission;
use iroha_schema::{Ident, IntoSchema};
use norito::json::{JsonDeserializeOwned, JsonSerialize};

/// Used to check if the permission token is owned by the account.
pub trait Permission: JsonSerialize + JsonDeserializeOwned + IntoSchema {
    /// Permission id, according to [`IntoSchema`].
    fn name() -> Ident {
        Self::type_name()
    }
}

macro_rules! permission {
    ($item:item) => {
        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            Permission,
            crate::json_macros::JsonSerialize,
            crate::json_macros::JsonDeserialize,
            iroha_schema::IntoSchema,
        )]
        $item
    };
}

/// Permission tokens related to peer management.
pub mod peer {
    use super::*;

    permission! {
        /// Permission allowing the peer manager to add or remove peers.
        #[derive(Copy)]
        pub struct CanManagePeers;
    }

    permission! {
        /// Permission allowing a multisig operator to manage lane-relay emergency rosters.
        #[derive(Copy)]
        pub struct CanManageLaneRelayEmergency;
    }
}

/// Permission tokens scoped to domains.
pub mod domain {
    use super::*;

    permission! {
        /// Permission to register a new domain.
        #[derive(Copy)]
        pub struct CanRegisterDomain;
    }

    permission! {
        /// Permission to unregister the specified domain.
        pub struct CanUnregisterDomain {
            /// Domain identifier governed by this permission.
            pub domain: DomainId,
        }
    }

    permission! {
        /// Permission to modify metadata for the specified domain.
        pub struct CanModifyDomainMetadata {
            /// Domain identifier whose metadata may be changed.
            pub domain: DomainId,
        }
    }
}

/// Permission tokens scoped to asset definitions.
pub mod asset_definition {
    use super::*;

    permission! {
        /// Permission to unregister the specified asset definition.
        pub struct CanUnregisterAssetDefinition {
            /// Identifier of the asset definition targeted by the permission.
            pub asset_definition: AssetDefinitionId,
        }
    }

    permission! {
        /// Permission to modify metadata for the specified asset definition.
        pub struct CanModifyAssetDefinitionMetadata {
            /// Identifier of the asset definition whose metadata may be changed.
            pub asset_definition: AssetDefinitionId,
        }
    }
}

/// Permission tokens scoped to accounts.
pub mod account {
    use super::*;

    /// Scope carried by account-alias permissions.
    #[derive(Debug, Clone, PartialEq, Eq, iroha_schema::IntoSchema)]
    #[allow(variant_size_differences)]
    #[norito(tag = "scope", content = "value", rename_all = "snake_case")]
    pub enum AccountAliasPermissionScope {
        /// Permission scoped to a specific dataspace-qualified domain.
        Domain(DomainId),
        /// Permission scoped to a dataspace alias segment.
        Dataspace(DataSpaceId),
    }

    impl norito::json::JsonSerialize for AccountAliasPermissionScope {
        fn json_serialize(&self, out: &mut String) {
            out.push_str("{\"scope\":\"");
            match self {
                Self::Domain(value) => {
                    out.push_str("domain\",\"value\":");
                    norito::json::JsonSerialize::json_serialize(value, out);
                }
                Self::Dataspace(value) => {
                    out.push_str("dataspace\",\"value\":");
                    norito::json::JsonSerialize::json_serialize(value, out);
                }
            }
            out.push('}');
        }
    }

    impl norito::json::JsonDeserialize for AccountAliasPermissionScope {
        fn json_deserialize(p: &mut norito::json::Parser<'_>) -> Result<Self, norito::json::Error> {
            let value =
                <norito::json::Value as norito::json::JsonDeserialize>::json_deserialize(p)?;
            Self::json_from_value(&value)
        }

        fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
            let object = value.as_object().ok_or_else(|| {
                norito::json::Error::Message("expected alias permission scope object".into())
            })?;
            let scope = object
                .get("scope")
                .and_then(norito::json::Value::as_str)
                .ok_or_else(|| {
                    norito::json::Error::Message(
                        "missing alias permission scope discriminator".into(),
                    )
                })?;
            let value = object.get("value").ok_or_else(|| {
                norito::json::Error::Message("missing alias permission scope value".into())
            })?;

            match scope {
                "domain" => Ok(Self::Domain(
                    <DomainId as norito::json::JsonDeserialize>::json_from_value(value)?,
                )),
                "dataspace" => Ok(Self::Dataspace(
                    <DataSpaceId as norito::json::JsonDeserialize>::json_from_value(value)?,
                )),
                other => Err(norito::json::Error::Message(format!(
                    "unknown alias permission scope `{other}`"
                ))),
            }
        }
    }

    permission! {
        /// Permission to register an account within the provided domain.
        pub struct CanRegisterAccount {
            /// Domain in which the account may be registered.
            pub domain: DomainId,
        }
    }

    permission! {
        /// Permission to unregister the specified account.
        pub struct CanUnregisterAccount {
            /// Identifier of the account targeted by the permission.
            pub account: AccountId,
        }
    }
    permission! {
        /// Permission to modify metadata for the specified account.
        pub struct CanModifyAccountMetadata {
            /// Identifier of the account whose metadata may be changed.
            pub account: AccountId,
        }
    }

    permission! {
        /// Permission to replace the controller for the specified account.
        pub struct CanReplaceAccountController {
            /// Identifier of the account whose controller may be replaced.
            pub account: AccountId,
        }
    }

    permission! {
        /// Permission to resolve account aliases in the specified scope.
        pub struct CanResolveAccountAlias {
            /// Alias permission scope.
            pub scope: AccountAliasPermissionScope,
        }
    }

    permission! {
        /// Permission to register, bind, or update account aliases in the specified scope.
        pub struct CanManageAccountAlias {
            /// Alias permission scope.
            pub scope: AccountAliasPermissionScope,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use iroha_schema::{IntoSchema, Metadata};

        #[test]
        fn alias_scope_serializes_as_snake_case() {
            let json =
                norito::json::to_json(&AccountAliasPermissionScope::Dataspace(DataSpaceId::GLOBAL))
                    .expect("serialize alias scope");

            assert_eq!(json, "{\"scope\":\"dataspace\",\"value\":0}");
        }

        #[test]
        fn alias_scope_deserializes_snake_case() {
            let scope: AccountAliasPermissionScope =
                norito::json::from_str("{\"scope\":\"dataspace\",\"value\":0}")
                    .expect("deserialize alias scope");

            assert_eq!(
                scope,
                AccountAliasPermissionScope::Dataspace(DataSpaceId::GLOBAL)
            );
        }

        #[test]
        fn alias_scope_schema_uses_snake_case_tags() {
            let schema = AccountAliasPermissionScope::schema();
            let Some(Metadata::Enum(meta)) = schema.get::<AccountAliasPermissionScope>() else {
                panic!("alias scope schema must be an enum");
            };

            let tags = meta
                .variants
                .iter()
                .map(|variant| variant.tag.as_str())
                .collect::<Vec<_>>();

            assert_eq!(tags, vec!["domain", "dataspace"]);
        }
    }
}

/// Permission tokens covering asset operations.
pub mod asset {
    use super::*;

    permission! {
        /// Permission to mint assets belonging to the specified definition.
        pub struct CanMintAssetWithDefinition {
            /// Definition identifier whose assets may be minted.
            pub asset_definition: AssetDefinitionId,
        }
    }

    permission! {
        /// Permission to burn assets belonging to the specified definition.
        pub struct CanBurnAssetWithDefinition {
            /// Definition identifier whose assets may be burned.
            pub asset_definition: AssetDefinitionId,
        }
    }

    permission! {
        /// Permission to transfer assets belonging to the specified definition.
        pub struct CanTransferAssetWithDefinition {
            /// Definition identifier whose assets may be transferred.
            pub asset_definition: AssetDefinitionId,
        }
    }

    permission! {
        /// Permission to mint the specified asset instance.
        pub struct CanMintAsset {
            /// Identifier of the asset instance that may be minted.
            pub asset: AssetId,
        }
    }

    permission! {
        /// Permission to burn the specified asset instance.
        pub struct CanBurnAsset {
            /// Identifier of the asset instance that may be burned.
            pub asset: AssetId,
        }
    }

    permission! {
        /// Permission to transfer the specified asset instance.
        pub struct CanTransferAsset {
            /// Identifier of the asset instance that may be transferred.
            pub asset: AssetId,
        }
    }

    permission! {
        /// Permission to modify metadata for assets belonging to the specified definition.
        pub struct CanModifyAssetMetadataWithDefinition {
            /// Definition identifier whose asset metadata may be changed.
            pub asset_definition: AssetDefinitionId,
        }
    }

    permission! {
        /// Permission to modify metadata for the specified asset instance.
        pub struct CanModifyAssetMetadata {
            /// Identifier of the asset instance whose metadata may be changed.
            pub asset: AssetId,
        }
    }
}

/// Permission tokens covering NFT operations.
pub mod nft {
    use super::*;

    permission! {
        /// Permission to register an NFT for the given domain.
        pub struct CanRegisterNft {
            /// Domain in which an NFT may be registered.
            pub domain: DomainId,
        }
    }

    permission! {
        /// Permission to unregister the specified NFT.
        pub struct CanUnregisterNft {
            /// Identifier of the NFT that may be unregistered.
            pub nft: NftId,
        }
    }

    permission! {
        /// Permission to transfer the specified NFT.
        pub struct CanTransferNft {
            /// Identifier of the NFT that may be transferred.
            pub nft: NftId,
        }
    }

    permission! {
        /// Permission to modify metadata for the specified NFT.
        pub struct CanModifyNftMetadata {
            /// Identifier of the NFT whose metadata may be changed.
            pub nft: NftId,
        }
    }
}

/// Permission tokens covering trigger management.
pub mod trigger {
    use super::*;

    permission! {
        /// Permission to register triggers on behalf of the provided authority.
        pub struct CanRegisterTrigger {
            /// Authority on whose behalf the trigger may be registered.
            pub authority: AccountId,
        }
    }

    permission! {
        /// Permission to unregister the specified trigger.
        pub struct CanUnregisterTrigger {
            /// Identifier of the trigger that may be removed.
            pub trigger: TriggerId,
        }
    }

    permission! {
        /// Permission to modify the configuration of a trigger.
        pub struct CanModifyTrigger {
            /// Identifier of the trigger that may be modified.
            pub trigger: TriggerId,
        }
    }

    permission! {
        /// Permission to execute a trigger manually.
        pub struct CanExecuteTrigger {
            /// Identifier of the trigger that may be executed.
            pub trigger: TriggerId,
        }
    }

    permission! {
        /// Permission to modify metadata of the specified trigger.
        pub struct CanModifyTriggerMetadata {
            /// Identifier of the trigger whose metadata may be changed.
            pub trigger: TriggerId,
        }
    }
}

/// Permission tokens for configuration parameters.
pub mod parameter {
    use super::*;

    permission! {
        /// Permission to set configuration parameters.
        #[derive(Copy)]
        pub struct CanSetParameters;
    }
}

/// Permission tokens affecting role lifecycle.
pub mod role {
    use super::*;

    permission! {
        /// Permission to manage role lifecycle.
        #[derive(Copy)]
        pub struct CanManageRoles;
    }
}

/// Permission tokens affecting executor upgrades.
pub mod executor {
    use super::*;

    permission! {
        /// Permission to upgrade the executor implementation.
        #[derive(Copy)]
        pub struct CanUpgradeExecutor;
    }
}

/// Smart contract related permissions
pub mod smart_contract {
    use super::*;

    permission! {
        /// Permission to register smart contract code artifacts.
        #[derive(Copy)]
        pub struct CanRegisterSmartContractCode;
    }
}

/// Nexus / Space Directory permissions.
pub mod nexus {
    use super::*;

    permission! {
        /// Permission to publish capability manifests for a dataspace.
        #[derive(Copy)]
        pub struct CanPublishSpaceDirectoryManifest {
            /// Dataspace identifier governed by the manifest.
            pub dataspace: DataSpaceId,
        }
    }

    permission! {
        /// Permission to charge Nexus fees to the specified sponsor account.
        pub struct CanUseFeeSponsor {
            /// Sponsor account that may be debited for fees.
            pub sponsor: AccountId,
        }
    }
}

/// Governance-related permissions
pub mod governance {
    use super::*;

    permission! {
        /// Allow proposing deployment of a smart contract via governance
        pub struct CanProposeContractDeployment {
            /// Canonical contract address targeted by the proposal.
            pub contract_address: ContractAddress,
        }
    }

    permission! {
        /// Allow submitting a governance ballot to a referendum/election
        pub struct CanSubmitGovernanceBallot {
            /// Referendum or election identifier (opaque string)
            pub referendum_id: String,
        }
    }

    permission! {
        /// Allow enacting an approved referendum
        #[derive(Copy)]
        pub struct CanEnactGovernance;
    }

    permission! {
        /// Allow managing sortition parliament parameters/membership
        #[derive(Copy)]
        pub struct CanManageParliament;
    }

    permission! {
        /// Allow recording citizen service discipline events.
        pub struct CanRecordCitizenService {
            /// Citizen account targeted by the record.
            pub owner: AccountId,
        }
    }

    permission! {
        /// Allow slashing governance bond locks for a referendum.
        pub struct CanSlashGovernanceLock {
            /// Referendum identifier (opaque string)
            pub referendum_id: String,
        }
    }

    permission! {
        /// Allow restituting governance bond locks after appeal.
        pub struct CanRestituteGovernanceLock {
            /// Referendum identifier (opaque string)
            pub referendum_id: String,
        }
    }
}

/// Permission tokens governing `SoraFS` operations.
pub mod sorafs {
    use super::*;
    use iroha_data_model::sorafs::prelude::ProviderId;

    permission! {
        /// Permission to register a `SoraFS` manifest pin.
        #[derive(Copy)]
        pub struct CanRegisterSorafsPin;
    }

    permission! {
        /// Permission to approve a `SoraFS` manifest pin.
        #[derive(Copy)]
        pub struct CanApproveSorafsPin;
    }

    permission! {
        /// Permission to retire a `SoraFS` manifest pin.
        #[derive(Copy)]
        pub struct CanRetireSorafsPin;
    }

    permission! {
        /// Permission to bind or update a `SoraFS` manifest alias.
        #[derive(Copy)]
        pub struct CanBindSorafsAlias;
    }

    permission! {
        /// Permission to declare storage capacity for a `SoraFS` provider.
        #[derive(Copy)]
        pub struct CanDeclareSorafsCapacity;
    }

    permission! {
        /// Permission to submit capacity telemetry for a `SoraFS` provider.
        #[derive(Copy)]
        pub struct CanSubmitSorafsTelemetry;
    }

    permission! {
        /// Permission to file a `SoraFS` capacity dispute.
        #[derive(Copy)]
        pub struct CanFileSorafsCapacityDispute;
    }

    permission! {
        /// Permission to issue `SoraFS` replication orders.
        #[derive(Copy)]
        pub struct CanIssueSorafsReplicationOrder;
    }

    permission! {
        /// Permission to complete `SoraFS` replication orders.
        #[derive(Copy)]
        pub struct CanCompleteSorafsReplicationOrder;
    }

    permission! {
        /// Permission to set `SoraFS` pricing schedules.
        #[derive(Copy)]
        pub struct CanSetSorafsPricing;
    }

    permission! {
        /// Permission to upsert `SoraFS` provider credit records.
        #[derive(Copy)]
        pub struct CanUpsertSorafsProviderCredit;
    }

    permission! {
        /// Permission to operate `SoraFS` repair tickets for a provider.
        #[derive(Copy)]
        pub struct CanOperateSorafsRepair {
            /// Provider identifier governed by this permission.
            pub provider_id: ProviderId,
        }
    }

    permission! {
        /// Permission to register or update a `SoraFS` provider owner binding.
        #[derive(Copy)]
        pub struct CanRegisterSorafsProviderOwner;
    }

    permission! {
        /// Permission to remove a `SoraFS` provider owner binding.
        #[derive(Copy)]
        pub struct CanUnregisterSorafsProviderOwner;
    }
}

/// Permission tokens governing `SoraNet` privacy ingestion.
pub mod soranet {
    use super::*;

    permission! {
        /// Permission to ingest `SoraNet` privacy events or shares.
        #[derive(Copy)]
        pub struct CanIngestSoranetPrivacy;
    }
}

#[cfg(test)]
mod tests {
    use super::account::CanRegisterAccount;

    #[test]
    fn can_register_account_serializes_as_json_string_field() {
        let perm = CanRegisterAccount {
            domain: DomainId::try_new("wonderland", "universal").expect("valid domain"),
        };

        let json = norito::json::to_json(&perm).expect("serialize to JSON");
        assert_eq!(json, "{\"domain\":\"wonderland\"}");

        let value = norito::json::to_value(&perm).expect("serialize to JSON value");
        assert_eq!(
            value,
            norito::json!({
                "domain": "wonderland",
            })
        );
    }
}
