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
        /// Permission scoped to one exact resolved account alias.
        Alias(ResolvedAccountAliasV1),
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
                Self::Alias(value) => {
                    out.push_str("alias\",\"value\":");
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
                "alias" => Ok(Self::Alias(
                    <ResolvedAccountAliasV1 as norito::json::JsonDeserialize>::json_from_value(
                        value,
                    )?,
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
        ///
        /// An alias scope authorizes one exact alias. A domain scope authorizes only that
        /// fully-qualified domain, while a dataspace scope authorizes only dataspace-root aliases;
        /// the three scope kinds are independent.
        pub struct CanResolveAccountAlias {
            /// Alias permission scope.
            pub scope: AccountAliasPermissionScope,
        }
    }

    permission! {
        /// Permission to grant or revoke alias-resolution access in the specified scope.
        ///
        /// Only the corresponding alias, domain, or dataspace-alias owner may grant or
        /// revoke this delegation token. Holding it authorizes delegation of
        /// [`CanResolveAccountAlias`] for the exact same scope; it never
        /// authorizes alias mutation or further delegation of this token.
        pub struct CanDelegateAccountAliasResolution {
            /// Exact alias scope whose resolution access may be delegated.
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
            let json = norito::json::to_json(&AccountAliasPermissionScope::Dataspace(
                DataSpaceId::UNIVERSAL,
            ))
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
                AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL)
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

            assert_eq!(tags, vec!["domain", "dataspace", "alias"]);
        }

        #[test]
        fn alias_scope_roundtrips_exact_resolved_alias() {
            let scope = AccountAliasPermissionScope::Alias(ResolvedAccountAliasV1::new(
                "merchant@banka.paynet"
                    .parse::<AccountAliasName>()
                    .expect("canonical account alias"),
                DataSpaceId::new(7),
            ));
            let json = norito::json::to_json(&scope).expect("serialize exact alias scope");
            let decoded: AccountAliasPermissionScope =
                norito::json::from_str(&json).expect("deserialize exact alias scope");

            assert_eq!(decoded, scope);
            assert!(json.contains("merchant"));
            assert!(json.contains("\"dataspace_id\":7"));
        }

        #[test]
        fn exact_alias_scope_matches_shared_alias_setup_fixture() {
            use norito::json::JsonDeserialize;

            let fixture: norito::json::Value = norito::json::from_str(include_str!(
                "../../../fixtures/norito_rpc/alias_setup_v1/alias_setup_v1.json"
            ))
            .expect("decode shared alias setup fixture");
            let raw_scope = fixture
                .as_object()
                .and_then(|object| object.get("permission_scope_json_vector"))
                .expect("shared exact alias permission scope");
            let scope = AccountAliasPermissionScope::json_from_value(raw_scope)
                .expect("decode shared exact alias permission scope");
            let expected = AccountAliasPermissionScope::Alias(ResolvedAccountAliasV1::new(
                "merchant@banka.paynet"
                    .parse::<AccountAliasName>()
                    .expect("canonical account alias"),
                DataSpaceId::new(7),
            ));

            assert_eq!(scope, expected);
            let encoded = norito::json::to_json(&scope).expect("encode shared exact alias scope");
            let encoded: norito::json::Value =
                norito::json::from_str(&encoded).expect("decode encoded exact alias scope");
            assert_eq!(&encoded, raw_scope);
        }

        #[test]
        fn alias_resolution_delegation_roundtrips_with_an_exact_scope() {
            let permission = CanDelegateAccountAliasResolution {
                scope: AccountAliasPermissionScope::Domain(
                    DomainId::try_new("hbl", "sbp").expect("canonical HBL domain"),
                ),
            };
            let encoded = norito::json::to_json(&permission)
                .expect("serialize alias-resolution delegation permission");
            let decoded: CanDelegateAccountAliasResolution = norito::json::from_str(&encoded)
                .expect("deserialize alias-resolution delegation permission");

            assert_eq!(decoded, permission);
            assert!(encoded.contains("hbl.sbp"));
        }
    }
}

/// Permission tokens governing reads from restricted Nexus dataspaces.
pub mod query {
    use super::*;

    permission! {
        /// Permission to read non-public ledger data from one exact dataspace.
        ///
        /// The token does not authorize writes, reads from any other dataspace,
        /// or account-alias resolution without its separate exact permission.
        #[derive(Copy)]
        pub struct CanReadRestrictedDataspace {
            /// Exact restricted dataspace whose ledger data may be read.
            pub dataspace: DataSpaceId,
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

    permission! {
        /// Permission to set or clear transfer-freeze state for one asset definition.
        pub struct CanSetAssetTransferFreeze {
            /// Asset definition whose account freeze state may be managed.
            pub asset_definition: AssetDefinitionId,
            /// Canonical on-chain alias domain of accounts whose freeze state may be managed.
            pub account_domain: iroha_data_model::account::rekey::AccountAliasDomain,
            /// Exact dataspace containing the canonical on-chain account alias.
            pub account_dataspace: iroha_data_model::nexus::DataSpaceId,
        }
    }

    permission! {
        /// Permission to set the daily transfer limit for one asset definition.
        pub struct CanSetAssetTransferDailyLimit {
            /// Asset definition whose daily account limits may be managed.
            pub asset_definition: AssetDefinitionId,
            /// Canonical on-chain alias domain of accounts whose daily limit may be managed.
            pub account_domain: iroha_data_model::account::rekey::AccountAliasDomain,
            /// Exact dataspace containing the canonical on-chain account alias.
            pub account_dataspace: iroha_data_model::nexus::DataSpaceId,
        }
    }
}

/// Permission tokens covering ZK-ACE identity management.
pub mod zk_ace {
    use super::*;

    permission! {
        /// Permission to manage ZK-ACE identity commitments for one source account and asset.
        pub struct CanManageZkAceIdentityForAccount {
            /// Source account whose ZK-ACE identity binding may be managed.
            pub account: AccountId,
            /// Asset definition governed by the identity binding.
            pub asset: AssetDefinitionId,
        }
    }
}

/// Permission tokens covering native asset escrow operations.
pub mod escrow {
    use super::*;

    permission! {
        /// Permission to resolve a disputed native asset escrow.
        #[derive(Copy)]
        pub struct CanResolveEscrowDispute;
    }
}

/// Permission tokens covering governed offline-settlement releases.
pub mod offline {
    use super::*;

    permission! {
        /// Permission to manage native offline escrow issuance and settlement.
        #[derive(Copy)]
        pub struct CanManageOfflineEscrow;
    }

    permission! {
        /// Permission to activate an authenticated Kagemusha ABI-21/V4 recursive release.
        #[derive(Copy)]
        pub struct CanActivateKagemushaRecursiveReleaseV4;
    }

    permission! {
        /// Permission to publish or rotate the governed offline device-attestation policy.
        #[derive(Copy)]
        pub struct CanManageOfflineDeviceAttestationPolicy;
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

/// Permission tokens for governed SCCP consensus state.
pub mod sccp {
    use super::*;

    permission! {
        /// Permission to enact governed SCCP registry actions.
        #[derive(Copy)]
        pub struct CanManageSccpGovernance;
    }

    permission! {
        /// Permission to submit typed SCCP route-governance proposals.
        #[derive(Copy)]
        pub struct CanProposeSccpRouteGovernance;
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

    permission! {
        /// Permission to invoke one exact entrypoint of one deployed contract instance.
        pub struct CanInvokeContractEntrypoint {
            /// Immutable deployed contract address.
            pub contract: ContractAddress,
            /// Exact case-sensitive public entrypoint selector.
            pub entrypoint: String,
        }
    }
}

/// Permission tokens governing native FX corridors.
pub mod settlement {
    use super::*;

    permission! {
        /// Root permission for delegating native FX corridor governance.
        #[derive(Copy)]
        pub struct CanManageFxCorridors;
    }

    permission! {
        /// Permission to publish the next revision of one FX corridor policy.
        pub struct CanSetFxCorridorPolicy {
            /// Stable corridor policy identifier.
            pub policy_id: Name,
        }
    }

    permission! {
        /// Permission to execute settlements under one FX corridor policy.
        pub struct CanSettleFxCorridor {
            /// Stable corridor policy identifier.
            pub policy_id: Name,
        }
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
        /// Permission to publish, replace, revoke, or expire one exact UAID manifest.
        #[derive(Copy)]
        pub struct CanPublishSpaceDirectoryManifestForUaid {
            /// Dataspace identifier governed by the manifest.
            pub dataspace: DataSpaceId,
            /// Exact universal account identifier governed by this permission.
            pub uaid: iroha_data_model::nexus::UniversalAccountId,
        }
    }

    permission! {
        /// Permission to manage manifests for accounts bound to one exact account-alias domain.
        pub struct CanPublishSpaceDirectoryManifestForAccountDomain {
            /// Dataspace identifier governed by the manifest.
            pub dataspace: DataSpaceId,
            /// Exact account-alias domain whose bound accounts may be managed.
            pub domain: DomainId,
        }
    }

    permission! {
        /// Permission to manage fee sponsor programs owned by one sponsor account.
        pub struct CanManageFeeSponsorProgram {
            /// Sponsor account whose programs may be created and revised.
            pub sponsor: AccountId,
        }
    }

    permission! {
        /// Permission to enroll or unenroll beneficiaries in one exact sponsor program.
        pub struct CanEnrollFeeSponsorProgram {
            /// Exact program whose enrollment set may be managed.
            pub program_id: FeeSponsorProgramId,
        }
    }

    permission! {
        /// Permission to withdraw assets from one exact sponsor-program vault.
        pub struct CanWithdrawFeeSponsorProgram {
            /// Exact program whose paused or closing vault may be withdrawn.
            pub program_id: FeeSponsorProgramId,
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
        /// Permission to configure, open, resolve, and finalize authoritative `SoraFS` moderation ballots.
        #[derive(Copy)]
        pub struct CanManageSorafsModeration;
    }

    permission! {
        /// Permission to activate or rotate the authoritative `SoraFS` `PoP` issuer policy.
        #[derive(Copy)]
        pub struct CanManageSorafsPopRegistry;
    }

    permission! {
        /// Permission to publish authoritative `SoraFS` `PoP` credential and revocation state.
        #[derive(Copy)]
        pub struct CanOperateSorafsPopIssuer;
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

/// Permission tokens governing `Musubi` package-registry operations.
pub mod musubi {
    use super::*;

    permission! {
        /// Permission to bind or update a curated global `Musubi` short alias.
        #[derive(Copy)]
        pub struct CanSetMusubiShortAlias;
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

/// Permission tokens governing operator-managed oracle state.
pub mod oracle {
    use super::*;
    use iroha_data_model::oracle::OracleChangeStage;

    permission! {
        /// Permission to register oracle feed configurations.
        #[derive(Copy)]
        pub struct CanRegisterOracleFeed;
    }

    permission! {
        /// Permission to propose oracle feed governance changes.
        #[derive(Copy)]
        pub struct CanProposeOracleChange;
    }

    permission! {
        /// Permission to vote in a specific oracle change stage.
        #[derive(Copy)]
        pub struct CanVoteOracleChangeStage {
            /// Stage in which the holder may vote.
            pub stage: OracleChangeStage,
        }
    }

    permission! {
        /// Permission to roll back oracle change proposals.
        #[derive(Copy)]
        pub struct CanRollbackOracleChange;
    }

    permission! {
        /// Permission to resolve oracle disputes.
        #[derive(Copy)]
        pub struct CanResolveOracleDispute;
    }

    permission! {
        /// Permission to record or revoke oracle-backed Twitter bindings.
        #[derive(Copy)]
        pub struct CanManageTwitterBindings;
    }
}

#[cfg(test)]
mod tests {
    use super::account::CanRegisterAccount;
    use super::asset::{CanSetAssetTransferDailyLimit, CanSetAssetTransferFreeze};
    use super::escrow::CanResolveEscrowDispute;
    use super::oracle::{
        CanManageTwitterBindings, CanRegisterOracleFeed, CanVoteOracleChangeStage,
    };
    use super::query::CanReadRestrictedDataspace;
    use crate::permission::Permission as _;
    use iroha_data_model::oracle::OracleChangeStage;
    use iroha_data_model::{
        DomainId, account::rekey::AccountAliasDomain, asset::AssetDefinitionId, nexus::DataSpaceId,
    };

    #[test]
    fn can_register_account_serializes_as_json_string_field() {
        let perm = CanRegisterAccount {
            domain: DomainId::try_new("wonderland", "universal").expect("valid domain"),
        };

        let json = norito::json::to_json(&perm).expect("serialize to JSON");
        assert_eq!(json, "{\"domain\":\"wonderland.universal\"}");

        let value = norito::json::to_value(&perm).expect("serialize to JSON value");
        assert_eq!(
            value,
            norito::json!({
                "domain": "wonderland.universal",
            })
        );
    }

    #[test]
    fn transfer_control_permissions_require_exact_domain_and_dataspace() {
        let asset_definition = AssetDefinitionId::new(
            DomainId::try_new("currency", "sbp").expect("asset domain"),
            "pkr".parse().expect("asset name"),
        );
        let account_domain = AccountAliasDomain::new("hbl".parse().expect("account domain"));
        let account_dataspace = DataSpaceId::new(10);
        let freeze = CanSetAssetTransferFreeze {
            asset_definition: asset_definition.clone(),
            account_domain: account_domain.clone(),
            account_dataspace,
        };
        let daily_limit = CanSetAssetTransferDailyLimit {
            asset_definition,
            account_domain,
            account_dataspace,
        };

        for value in [
            norito::json::to_value(&freeze).expect("freeze permission JSON"),
            norito::json::to_value(&daily_limit).expect("daily-limit permission JSON"),
        ] {
            let object = value.as_object().expect("permission payload object");
            assert_eq!(
                object.len(),
                3,
                "permission payload must have no hidden fields"
            );
            assert_eq!(object["account_domain"].as_str(), Some("hbl"));
            assert_eq!(object["account_dataspace"].as_u64(), Some(10));
        }

        let missing_dataspace = norito::json!({
            "asset_definition": (freeze.asset_definition.to_string()),
            "account_domain": "hbl",
        });
        assert!(
            norito::json::from_value::<CanSetAssetTransferFreeze>(missing_dataspace).is_err(),
            "first-release permission decoding must not accept domain-only legacy payloads",
        );
        let alias_string_dataspace = norito::json!({
            "asset_definition": (freeze.asset_definition.to_string()),
            "account_domain": "hbl",
            "account_dataspace": "sbp",
        });
        assert!(
            norito::json::from_value::<CanSetAssetTransferFreeze>(alias_string_dataspace).is_err(),
            "typed permission must require numeric DataSpaceId, not a browser alias",
        );
    }

    #[test]
    fn escrow_court_permission_uses_expected_name() {
        assert_eq!(
            CanResolveEscrowDispute::name().as_str(),
            "CanResolveEscrowDispute"
        );
    }

    #[test]
    fn restricted_dataspace_read_permission_is_exact_and_typed() {
        let permission = CanReadRestrictedDataspace {
            dataspace: DataSpaceId::new(10),
        };
        let json = norito::json::to_json(&permission).expect("serialize permission");

        assert_eq!(
            CanReadRestrictedDataspace::name().as_str(),
            "CanReadRestrictedDataspace"
        );
        assert_eq!(json, "{\"dataspace\":10}");
        assert!(
            norito::json::from_str::<CanReadRestrictedDataspace>("{\"dataspace\":\"sbp\"}")
                .is_err(),
            "restricted read grants must carry a numeric DataSpaceId"
        );
    }

    #[test]
    fn oracle_permissions_use_expected_names_and_payloads() {
        assert_eq!(
            CanRegisterOracleFeed::name().as_str(),
            "CanRegisterOracleFeed"
        );
        assert_eq!(
            CanManageTwitterBindings::name().as_str(),
            "CanManageTwitterBindings"
        );

        let stage_vote = CanVoteOracleChangeStage {
            stage: OracleChangeStage::PolicyJury,
        };
        let json = norito::json::to_json(&stage_vote).expect("serialize to JSON");
        assert!(json.contains("PolicyJury"));
    }
}
