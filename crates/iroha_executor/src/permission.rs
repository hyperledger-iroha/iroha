//! Module with permission related functionality.
//!
//! Post-genesis delegation normally lets an authority propagate an exact permission it holds,
//! exercise a native use-time ownership root for that same capability, or use an explicitly wider
//! parent permission. Exact asset-definition-alias grants instead require both the active asset
//! owner and namespace authority; after clear, only the native namespace root can revoke. The
//! leaf token pins its definition so it cannot migrate across a label rebind. Bootstrap-root
//! permissions remain genesis-only, and ownership of an adjacent field grants no authority.
use crate::{
    Execute,
    prelude::Context,
    smart_contract::{
        data_model::{executor::Result, permission::Permission as PermissionObject, prelude::*},
        prelude::*,
    },
};
use iroha_executor_data_model::permission::{
    Permission, asset_definition::CanManageAssetDefinitionAlias,
};
use std::{borrow::ToOwned as _, collections::BTreeSet, vec::Vec};
#[cfg(test)]
pub(crate) mod test_override {
    use std::cell::RefCell;
    thread_local! {
        static PERMISSIONS: RefCell<Vec<crate::data_model::permission::Permission>> = const {
            RefCell::new(Vec::new())
        };
    }
    pub fn permissions() -> Vec<crate::data_model::permission::Permission> {
        PERMISSIONS.with(|permissions| permissions.borrow().clone())
    }
    pub fn replace_permissions(
        permissions: Vec<crate::data_model::permission::Permission>,
    ) -> Vec<crate::data_model::permission::Permission> {
        PERMISSIONS.with(|current| {
            let mut current = current.borrow_mut();
            core::mem::replace(&mut *current, permissions)
        })
    }
}
/// Declare permission types of current module. Use it with a full path to the permission.
/// Used to iterate over tokens to validate `Grant` and `Revoke` instructions.
///
///
/// Example:
///
/// ```ignore
/// mod tokens {
///     use std::borrow::ToOwned;
///
///     use iroha_schema::IntoSchema;
///     use iroha_executor_derive::Permission;
///
///     use iroha_executor_data_model::json_macros::{JsonDeserialize, JsonSerialize};
///
///     #[derive(
///         Clone,
///         PartialEq,
///         JsonDeserialize,
///         JsonSerialize,
///         IntoSchema,
///         Permission,
///     )]
///     #[validate(iroha_executor::permission::OnlyGenesis)]
///     pub struct MyToken;
/// }
/// ```
macro_rules! declare_permissions {
    ($($($token_path:ident ::)+ { $token_ty:ident }),+ $(,)?) => {
        /// Enum with every default permission
        #[allow(clippy::enum_variant_names)]
        #[derive(Clone)]
        pub(crate) enum AnyPermission { $(
            $token_ty($($token_path::)+$token_ty), )*
        }
        impl TryFrom<&PermissionObject> for AnyPermission {
            type Error = iroha_executor_data_model::TryFromDataModelObjectError;
            fn try_from(permission: &PermissionObject) -> Result<Self, Self::Error> {
                match permission.name().as_ref() { $(
                    stringify!($token_ty) => {
                        let permission = <$($token_path::)+$token_ty>::try_from(permission)?;
                        Ok(Self::$token_ty(permission))
                    } )+
                    _ => Err(Self::Error::UnknownIdent(permission.name().to_owned()))
                }
            }
        }
        impl From<AnyPermission> for PermissionObject {
            fn from(permission: AnyPermission) -> Self {
                match permission { $(
                    AnyPermission::$token_ty(permission) => permission.into(), )*
                }
            }
        }
        impl ValidateGrantRevoke for AnyPermission {
            fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
                self.validate_payload()?;
                if self.is_holder_delegable() && self.is_owned_by(authority, host) {
                    return Ok(());
                }
                match self { $(
                    AnyPermission::$token_ty(permission) => permission.validate_grant(authority, context, host), )*
                }
            }
            fn validate_revoke(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
                self.validate_payload()?;
                if self.is_holder_delegable() && self.is_owned_by(authority, host) {
                    return Ok(());
                }
                match self { $(
                    AnyPermission::$token_ty(permission) => permission.validate_revoke(authority, context, host), )*
                }
            }
        }
        impl AnyPermission {
            fn is_owned_by(&self, authority: &AccountId, host: &Iroha) -> bool {
                match self { $(
                    AnyPermission::$token_ty(permission) => permission.is_owned_by(authority, host), )*
                }
            }
        }
        macro_rules! map_default_permissions {
            ($callback:ident) => { $(
                $callback!($($token_path::)+$token_ty); )+
            };
        }

        pub(crate) use map_default_permissions;
    };
}
declare_permissions! {
    iroha_executor_data_model::permission::peer::{CanManagePeers},
    iroha_executor_data_model::permission::peer::{CanManageLaneRelayEmergency},
    iroha_executor_data_model::permission::domain::{CanRegisterDomain},
    iroha_executor_data_model::permission::domain::{CanUnregisterDomain},
    iroha_executor_data_model::permission::domain::{CanModifyDomainMetadata},
    iroha_executor_data_model::permission::account::{CanRegisterAccount},
    iroha_executor_data_model::permission::account::{CanUnregisterAccount},
    iroha_executor_data_model::permission::account::{CanModifyAccountMetadata},
    iroha_executor_data_model::permission::account::{CanReplaceAccountController},
    iroha_executor_data_model::permission::account::{CanManageAccountAlias},
    iroha_executor_data_model::permission::account::{CanDelegateAccountAliasResolution},
    iroha_executor_data_model::permission::account::{CanResolveAccountAlias},
    iroha_executor_data_model::permission::query::{CanReadRestrictedDataspace},
    iroha_executor_data_model::permission::query::{CanReadAllLedgerData},
    iroha_executor_data_model::permission::query::{CanReadAccountData},
    iroha_executor_data_model::permission::asset_definition::{CanUnregisterAssetDefinition},
    iroha_executor_data_model::permission::asset_definition::{CanModifyAssetDefinitionMetadata},
    iroha_executor_data_model::permission::asset_definition::{CanManageAssetDefinitionConfidentialPolicy},
    iroha_executor_data_model::permission::asset_definition::{CanManageAssetDefinitionAlias},
    iroha_executor_data_model::permission::asset::{CanMintAssetWithDefinition},
    iroha_executor_data_model::permission::asset::{CanBurnAssetWithDefinition},
    iroha_executor_data_model::permission::asset::{CanTransferAssetWithDefinition},
    iroha_executor_data_model::permission::asset::{CanModifyAssetMetadataWithDefinition},
    iroha_executor_data_model::permission::asset::{CanMintAssetToAccount},
    iroha_executor_data_model::permission::asset::{CanBurnAsset},
    iroha_executor_data_model::permission::asset::{CanTransferAsset},
    iroha_executor_data_model::permission::asset::{CanModifyAssetMetadata},
    iroha_executor_data_model::permission::asset::{CanSetAssetTransferAvailability},
    iroha_executor_data_model::permission::asset::{CanSetAssetTransferDailyLimit},
    iroha_executor_data_model::permission::asset::{CanSetAssetHoldingLimit},
    iroha_executor_data_model::permission::nft::{CanRegisterNft},
    iroha_executor_data_model::permission::nft::{CanUnregisterNft},
    iroha_executor_data_model::permission::nft::{CanTransferNft},
    iroha_executor_data_model::permission::nft::{CanModifyNftMetadata},
    iroha_executor_data_model::permission::parameter::{CanSetParameters},
    iroha_executor_data_model::permission::sccp::{CanManageSccpGovernance},
    iroha_executor_data_model::permission::sccp::{CanProposeSccpRouteGovernance},
    iroha_executor_data_model::permission::offline::{CanManageOfflineEscrow},
    iroha_executor_data_model::permission::offline::{CanActivateKagemushaRecursiveReleaseV4},
    iroha_executor_data_model::permission::offline::{CanManageOfflineDeviceAttestationPolicy},
    iroha_executor_data_model::permission::role::{CanManageRoles},
    iroha_executor_data_model::permission::trigger::{CanRegisterTrigger},
    iroha_executor_data_model::permission::trigger::{CanUnregisterTrigger},
    iroha_executor_data_model::permission::trigger::{CanModifyTrigger},
    iroha_executor_data_model::permission::trigger::{CanExecuteTrigger},
    iroha_executor_data_model::permission::trigger::{CanModifyTriggerMetadata},
    iroha_executor_data_model::permission::executor::{CanUpgradeExecutor},
    iroha_executor_data_model::permission::governance::{CanManageRuntimeUpgrades},
    iroha_executor_data_model::permission::governance::{CanManageConsensusKeys},
    iroha_executor_data_model::permission::governance::{CanManageConfidentialParams},
    iroha_executor_data_model::permission::smart_contract::{CanRegisterSmartContractCode},
    iroha_executor_data_model::permission::smart_contract::{CanInvokeContractEntrypoint},
    iroha_executor_data_model::permission::settlement::{CanExecuteSettlement},
    iroha_executor_data_model::permission::settlement::{CanManageFxCorridors},
    iroha_executor_data_model::permission::settlement::{CanSetFxCorridorPolicy},
    iroha_executor_data_model::permission::dpn::{DpnAdmin},
    iroha_executor_data_model::permission::dpn::{DpnUser},
    iroha_executor_data_model::permission::dpn::{DpnInori},
    iroha_executor_data_model::permission::dpn::{DpnSettlement},
    iroha_executor_data_model::permission::dpn::{DpnEprGuard},
    iroha_executor_data_model::permission::sorafs::{CanBindSorafsAlias},
    iroha_executor_data_model::permission::sorafs::{CanDeclareSorafsCapacity},
    iroha_executor_data_model::permission::sorafs::{CanSubmitSorafsTelemetry},
    iroha_executor_data_model::permission::sorafs::{CanFileSorafsCapacityDispute},
    iroha_executor_data_model::permission::sorafs::{CanIssueSorafsReplicationOrder},
    iroha_executor_data_model::permission::sorafs::{CanCompleteSorafsReplicationOrder},
    iroha_executor_data_model::permission::sorafs::{CanSetSorafsPricing},
    iroha_executor_data_model::permission::sorafs::{CanSetSorafsReservePolicy},
    iroha_executor_data_model::permission::sorafs::{CanManageSorafsModeration},
    iroha_executor_data_model::permission::sorafs::{CanManageSorafsPopRegistry},
    iroha_executor_data_model::permission::sorafs::{CanOperateSorafsPopIssuer},
    iroha_executor_data_model::permission::sorafs::{CanUpsertSorafsProviderCredit},
    iroha_executor_data_model::permission::soranet::{CanManageSoranetVpnQuoteIssuers},
    iroha_executor_data_model::permission::soranet::{CanIssueSoranetVpnQuote},
    iroha_executor_data_model::permission::soranet::{CanIngestSoranetPrivacy},
    iroha_executor_data_model::permission::oracle::{CanRegisterOracleFeed},
    iroha_executor_data_model::permission::oracle::{CanProposeOracleChange},
    iroha_executor_data_model::permission::oracle::{CanVoteOracleChangeStage},
    iroha_executor_data_model::permission::oracle::{CanRollbackOracleChange},
    iroha_executor_data_model::permission::oracle::{CanResolveOracleDispute},
    iroha_executor_data_model::permission::oracle::{CanManageTwitterBindings},
    iroha_executor_data_model::permission::nexus::{CanPublishSpaceDirectoryManifest},
    iroha_executor_data_model::permission::nexus::{CanPublishSpaceDirectoryManifestForAccountDomain},
    iroha_executor_data_model::permission::nexus::{CanPublishSpaceDirectoryManifestForUaid},
    iroha_executor_data_model::permission::nexus::{CanManageFeeSponsorProgram},
    iroha_executor_data_model::permission::nexus::{CanEnrollFeeSponsorProgram},
}
impl AnyPermission {
    /// Return whether this is one of the account-bound NEVO DPN application roles.
    pub(crate) fn is_dpn_application_permission(&self) -> bool {
        matches!(
            self,
            Self::DpnAdmin(_)
                | Self::DpnUser(_)
                | Self::DpnInori(_)
                | Self::DpnSettlement(_)
                | Self::DpnEprGuard(_)
        )
    }

    /// Return whether this permission is immutable after genesis.
    ///
    /// Exact possession never bypasses this list: these capabilities are bootstrap roots, not
    /// recursively delegable tokens.
    fn is_genesis_only(&self) -> bool {
        matches!(
            self,
            Self::CanManagePeers(_)
                | Self::CanManageLaneRelayEmergency(_)
                | Self::CanRegisterDomain(_)
                | Self::CanReadAllLedgerData(_)
                | Self::CanReadRestrictedDataspace(_)
                | Self::CanManageOfflineEscrow(_)
                | Self::CanActivateKagemushaRecursiveReleaseV4(_)
                | Self::CanManageOfflineDeviceAttestationPolicy(_)
                | Self::CanManageRoles(_)
                | Self::CanUpgradeExecutor(_)
                | Self::CanRegisterSmartContractCode(_)
                | Self::CanManageFxCorridors(_)
        )
    }
    /// Exact account-read holders may use their grant but cannot propagate it: only the
    /// account named by the token controls its lifecycle. Exact asset-definition-alias holders
    /// likewise cannot bypass the asset-owner plus namespace-authority grant rule. Genesis-only
    /// roots are non-delegable after bootstrap.
    fn is_holder_delegable(&self) -> bool {
        !self.is_genesis_only()
            && !matches!(
                self,
                Self::CanReadAccountData(_)
                    | Self::CanIssueSoranetVpnQuote(_)
                    | Self::DpnAdmin(_)
                    | Self::DpnUser(_)
                    | Self::DpnInori(_)
                    | Self::DpnSettlement(_)
                    | Self::DpnEprGuard(_)
                    | Self::CanManageAssetDefinitionAlias(CanManageAssetDefinitionAlias {
                        scope: iroha_executor_data_model::permission::asset_definition::AssetDefinitionAliasPermissionScope::Alias(_),
                    })
            )
    }
    fn validate_payload(&self) -> Result {
        match self {
            Self::CanInvokeContractEntrypoint(permission) => {
                smart_contract::validate_contract_entrypoint_payload(permission)
            }
            _ => Ok(()),
        }
    }
}
macro_rules! impl_validate_grant_revoke_via {
    ($provider:path => $($permission:ty),+ $(,)?) => {
        $(
            impl ValidateGrantRevoke for $permission {
                fn validate_grant(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    $provider(self).validate(authority, host, context)
                }

                fn validate_revoke(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    $provider(self).validate(authority, host, context)
                }
            }
        )+
    };
}
mod query {
    use super::*;
    use iroha_executor_data_model::permission::query::{
        CanReadAllLedgerData, CanReadRestrictedDataspace,
    };
    impl_validate_grant_revoke_via!(OnlyGenesis::from =>
        CanReadRestrictedDataspace,
        CanReadAllLedgerData,
    );
}
/// Trait that enables using permissions on the blockchain
pub trait ExecutorPermission: Permission + PartialEq {
    /// Check if the account owns this permission
    fn is_owned_by(&self, authority: &AccountId, host: &Iroha) -> bool
    where
        for<'a> Self: TryFrom<&'a crate::data_model::permission::Permission>,
    {
        #[cfg(test)]
        {
            let override_permissions = test_override::permissions();
            if !override_permissions.is_empty() {
                return has_permission_in_account(&override_permissions, self);
            }
        }
        let account_permissions: Vec<_> = host
            .query(FindPermissionsByAccountId::new(authority.clone()))
            .execute()
            .expect("INTERNAL BUG: `FindPermissionsByAccountId` must never fail")
            .map(|res| res.dbg_expect("Failed to get permission from cursor"))
            .collect();
        if has_permission_in_account(&account_permissions, self) {
            return true;
        }
        // collect all roles assigned to the authority
        let role_ids: Vec<RoleId> = host
            .query(FindRolesByAccountId::new(authority.clone()))
            .execute()
            .expect("INTERNAL BUG: `FindRolesByAccountId` must never fail")
            .map(|role_id| role_id.dbg_expect("Failed to get role from cursor"))
            .collect();
        // check if any of the roles have the permission we need
        if role_ids.is_empty() {
            return false;
        }
        let role_permissions: Vec<_> = roles_permissions(host).collect();
        permission_owned_in_sources(&account_permissions, &role_permissions, &role_ids, self)
    }
}
impl<T: Permission + PartialEq> ExecutorPermission for T {}
/// Trait that should be implemented for all permission tokens. Provides a function to check
/// validity of [`Grant`] and [`Revoke`] instructions containing implementing permission.
pub(super) trait ValidateGrantRevoke {
    fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result;
    fn validate_revoke(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result;
}
/// Predicate-like trait used for pass conditions to identify if [`Grant`] or [`Revoke`] should be allowed.
pub(crate) trait PassCondition {
    /// Validate whether the condition permits the grant or revoke operation.
    ///
    /// # Errors
    /// Returns an error if the authority or role context does not satisfy the condition
    /// or if validation fails due to host lookups.
    fn validate(&self, authority: &AccountId, host: &Iroha, context: &Context) -> Result;
}
fn ensure_permission_owned<T>(
    permission: &T,
    authority: &AccountId,
    host: &Iroha,
    permission_name: &str,
) -> Result
where
    T: ExecutorPermission,
    for<'a> T: TryFrom<&'a crate::data_model::permission::Permission>,
{
    if permission.is_owned_by(authority, host) {
        Ok(())
    } else {
        Err(ValidationFail::NotPermitted(format!(
            "Current authority doesn't have the {permission_name} permission, therefore it can't grant or revoke it"
        )))
    }
}
macro_rules! impl_owned_permission {
    ($($ty:ty),+ $(,)?) => {$(
        impl ValidateGrantRevoke for $ty {
            fn validate_grant(
                &self,
                authority: &AccountId,
                _context: &Context,
                host: &Iroha,
            ) -> Result {
                super::ensure_permission_owned(self, authority, host, stringify!($ty))
            }
            fn validate_revoke(
                &self,
                authority: &AccountId,
                _context: &Context,
                host: &Iroha,
            ) -> Result {
                super::ensure_permission_owned(self, authority, host, stringify!($ty))
            }
        }
    )+};
}
mod executor {
    use super::*;
    use iroha_executor_data_model::permission::executor::CanUpgradeExecutor;
    impl_validate_grant_revoke_via!(OnlyGenesis::from => CanUpgradeExecutor);
}
mod governance {
    use super::*;
    use iroha_executor_data_model::permission::governance::{
        CanManageConfidentialParams, CanManageConsensusKeys, CanManageRuntimeUpgrades,
    };

    impl_owned_permission!(
        CanManageRuntimeUpgrades,
        CanManageConsensusKeys,
        CanManageConfidentialParams,
    );
}
mod smart_contract {
    use super::*;
    use iroha_executor_data_model::permission::smart_contract::{
        CanInvokeContractEntrypoint, CanRegisterSmartContractCode,
    };
    impl_validate_grant_revoke_via!(OnlyGenesis::from => CanRegisterSmartContractCode);
    pub(super) fn validate_contract_entrypoint_payload(
        permission: &CanInvokeContractEntrypoint,
    ) -> Result {
        let entrypoint = permission.entrypoint.as_str();
        if entrypoint.is_empty() || entrypoint.trim() != entrypoint {
            return Err(ValidationFail::NotPermitted(
                "contract entrypoint permission must use a non-empty canonical selector".to_owned(),
            ));
        }
        Ok(())
    }
    fn validate_contract_entrypoint_delegation(
        permission: &CanInvokeContractEntrypoint,
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        validate_contract_entrypoint_payload(permission)?;
        if context.curr_block.is_genesis()
            || CanRegisterSmartContractCode.is_owned_by(authority, host)
        {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "only genesis, an exact holder, or a smart-contract registrar may delegate an exact contract entrypoint permission"
                .to_owned(),
        ))
    }
    impl ValidateGrantRevoke for CanInvokeContractEntrypoint {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            validate_contract_entrypoint_delegation(self, authority, context, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            validate_contract_entrypoint_delegation(self, authority, context, host)
        }
    }
}
mod settlement {
    use super::*;
    use iroha_executor_data_model::permission::settlement::{
        CanExecuteSettlement, CanManageFxCorridors, CanSetFxCorridorPolicy,
    };
    impl_validate_grant_revoke_via!(OnlyGenesis::from => CanManageFxCorridors);
    fn validate_bilateral_settlement_consent(
        permission: &CanExecuteSettlement,
        authority: &AccountId,
        context: &Context,
    ) -> Result {
        if context.curr_block.is_genesis() || permission.debited_asset.account() == authority {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "only the debited account may delegate or revoke exact bilateral settlement consent"
                .to_owned(),
        ))
    }
    impl ValidateGrantRevoke for CanExecuteSettlement {
        fn validate_grant(
            &self,
            authority: &AccountId,
            context: &Context,
            _host: &Iroha,
        ) -> Result {
            validate_bilateral_settlement_consent(self, authority, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            _host: &Iroha,
        ) -> Result {
            validate_bilateral_settlement_consent(self, authority, context)
        }
    }
    fn validate_corridor_delegation(
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        if context.curr_block.is_genesis() || CanManageFxCorridors.is_owned_by(authority, host) {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "only genesis or an FX corridor manager may delegate corridor permissions".to_owned(),
        ))
    }
    macro_rules! impl_corridor_permission {
        ($ty:ty) => {
            impl ValidateGrantRevoke for $ty {
                fn validate_grant(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    validate_corridor_delegation(authority, context, host)
                }
                fn validate_revoke(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    validate_corridor_delegation(authority, context, host)
                }
            }
        };
    }
    impl_corridor_permission!(CanSetFxCorridorPolicy);
}
mod dpn {
    use super::*;
    use iroha_executor_data_model::permission::dpn::{
        DpnAdmin, DpnEprGuard, DpnInori, DpnSettlement, DpnUser,
    };

    fn is_direct_dpn_admin(authority: &AccountId, host: &Iroha) -> bool {
        #[cfg(test)]
        {
            let override_permissions = test_override::permissions();
            if !override_permissions.is_empty() {
                return has_permission_in_account(&override_permissions, &DpnAdmin);
            }
        }
        let account_permissions: Vec<_> = host
            .query(FindPermissionsByAccountId::new(authority.clone()))
            .execute()
            .expect("INTERNAL BUG: `FindPermissionsByAccountId` must never fail")
            .map(|result| result.dbg_expect("Failed to get permission from cursor"))
            .collect();
        has_permission_in_account(&account_permissions, &DpnAdmin)
    }

    fn validate_dpn_role_lifecycle(
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        if context.curr_block.is_genesis() || is_direct_dpn_admin(authority, host) {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "only genesis or an exact DpnAdmin holder may grant or revoke NEVO DPN permissions"
                .to_owned(),
        ))
    }

    macro_rules! impl_dpn_permission {
        ($ty:ty) => {
            impl ValidateGrantRevoke for $ty {
                fn validate_grant(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    validate_dpn_role_lifecycle(authority, context, host)
                }

                fn validate_revoke(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    validate_dpn_role_lifecycle(authority, context, host)
                }
            }
        };
    }

    impl_dpn_permission!(DpnAdmin);
    impl_dpn_permission!(DpnUser);
    impl_dpn_permission!(DpnInori);
    impl_dpn_permission!(DpnSettlement);
    impl_dpn_permission!(DpnEprGuard);
}
mod nexus {
    use super::*;
    use iroha_executor_data_model::permission::nexus::{
        CanEnrollFeeSponsorProgram, CanManageFeeSponsorProgram, CanPublishSpaceDirectoryManifest,
        CanPublishSpaceDirectoryManifestForAccountDomain, CanPublishSpaceDirectoryManifestForUaid,
    };
    fn ensure_publish_manifest_grant_authority(
        permission: CanPublishSpaceDirectoryManifest,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result {
        #[cfg(test)]
        {
            let override_permissions = test_override::permissions();
            if !override_permissions.is_empty() {
                if has_permission_in_account(&override_permissions, &permission) {
                    return Ok(());
                }
                return Err(ValidationFail::NotPermitted(
                    "Current authority doesn't have the CanPublishSpaceDirectoryManifest permission, therefore it can't grant or revoke it"
                        .to_owned(),
                ));
            }
        }
        if permission.is_owned_by(authority, host) {
            Ok(())
        } else {
            Err(ValidationFail::NotPermitted(
                "Current authority doesn't have the CanPublishSpaceDirectoryManifest permission, therefore it can't grant or revoke it"
                    .to_owned(),
            ))
        }
    }
    impl ValidateGrantRevoke for CanPublishSpaceDirectoryManifest {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            if context.curr_block.is_genesis() {
                return Ok(());
            }
            ensure_publish_manifest_grant_authority(*self, authority, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            if context.curr_block.is_genesis() {
                return Ok(());
            }
            ensure_publish_manifest_grant_authority(*self, authority, host)
        }
    }
    fn ensure_uaid_manifest_grant_authority(
        permission: CanPublishSpaceDirectoryManifestForUaid,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result {
        if permission.is_owned_by(authority, host)
            || (CanPublishSpaceDirectoryManifest {
                dataspace: permission.dataspace,
            })
            .is_owned_by(authority, host)
        {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "Current authority must hold either the exact UAID-scoped permission or the dataspace-wide CanPublishSpaceDirectoryManifest permission to grant or revoke it"
                .to_owned(),
        ))
    }
    impl ValidateGrantRevoke for CanPublishSpaceDirectoryManifestForUaid {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            if context.curr_block.is_genesis() {
                return Ok(());
            }
            ensure_uaid_manifest_grant_authority(*self, authority, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            if context.curr_block.is_genesis() {
                return Ok(());
            }
            ensure_uaid_manifest_grant_authority(*self, authority, host)
        }
    }
    fn ensure_account_domain_manifest_grant_authority(
        permission: &CanPublishSpaceDirectoryManifestForAccountDomain,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result {
        #[cfg(test)]
        {
            let override_permissions = test_override::permissions();
            if !override_permissions.is_empty() {
                if has_permission_in_account(&override_permissions, permission)
                    || has_permission_in_account(
                        &override_permissions,
                        &CanPublishSpaceDirectoryManifest {
                            dataspace: permission.dataspace,
                        },
                    )
                {
                    return Ok(());
                }
                return Err(ValidationFail::NotPermitted(
                    "Test authority does not hold the exact account-domain or dataspace-wide manifest permission"
                        .to_owned(),
                ));
            }
        }
        if permission.is_owned_by(authority, host)
            || (CanPublishSpaceDirectoryManifest {
                dataspace: permission.dataspace,
            })
            .is_owned_by(authority, host)
        {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "Only an existing exact holder or a dataspace-wide manifest authority may grant or revoke this permission"
                .to_owned(),
        ))
    }
    impl ValidateGrantRevoke for CanPublishSpaceDirectoryManifestForAccountDomain {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            if context.curr_block.is_genesis() {
                return Ok(());
            }
            ensure_account_domain_manifest_grant_authority(self, authority, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            if context.curr_block.is_genesis() {
                return Ok(());
            }
            ensure_account_domain_manifest_grant_authority(self, authority, host)
        }
    }
    fn ensure_sponsor_program_delegation_authority(
        sponsor: &AccountId,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result {
        if authority == sponsor
            || (CanManageFeeSponsorProgram {
                sponsor: sponsor.clone(),
            })
            .is_owned_by(authority, host)
        {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "only the sponsor or its fee-program manager may delegate this permission".to_owned(),
        ))
    }
    impl ValidateGrantRevoke for CanManageFeeSponsorProgram {
        fn validate_grant(
            &self,
            authority: &AccountId,
            context: &Context,
            _host: &Iroha,
        ) -> Result {
            if context.curr_block.is_genesis() || authority == &self.sponsor {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "only the sponsor account may delegate fee-program management".to_owned(),
            ))
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            _host: &Iroha,
        ) -> Result {
            if context.curr_block.is_genesis() {
                return Ok(());
            }
            if authority == &self.sponsor {
                Ok(())
            } else {
                Err(ValidationFail::NotPermitted(
                    "only the sponsor account may revoke fee-program management".to_owned(),
                ))
            }
        }
    }
    macro_rules! impl_program_scoped_delegation {
        ($permission:ty) => {
            impl ValidateGrantRevoke for $permission {
                fn validate_grant(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    if context.curr_block.is_genesis() {
                        return Ok(());
                    }
                    ensure_sponsor_program_delegation_authority(
                        &self.program_id.sponsor,
                        authority,
                        host,
                    )
                }
                fn validate_revoke(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    if context.curr_block.is_genesis() {
                        return Ok(());
                    }
                    ensure_sponsor_program_delegation_authority(
                        &self.program_id.sponsor,
                        authority,
                        host,
                    )
                }
            }
        };
    }
    impl_program_scoped_delegation!(CanEnrollFeeSponsorProgram);
}
mod sorafs {
    use super::*;
    use iroha_executor_data_model::permission::sorafs::{
        CanBindSorafsAlias, CanCompleteSorafsReplicationOrder, CanDeclareSorafsCapacity,
        CanFileSorafsCapacityDispute, CanIssueSorafsReplicationOrder, CanManageSorafsModeration,
        CanManageSorafsPopRegistry, CanOperateSorafsPopIssuer, CanSetSorafsPricing,
        CanSetSorafsReservePolicy, CanSubmitSorafsTelemetry, CanUpsertSorafsProviderCredit,
    };
    impl_owned_permission!(
        CanBindSorafsAlias,
        CanDeclareSorafsCapacity,
        CanSubmitSorafsTelemetry,
        CanFileSorafsCapacityDispute,
        CanIssueSorafsReplicationOrder,
        CanCompleteSorafsReplicationOrder,
        CanManageSorafsModeration,
        CanManageSorafsPopRegistry,
        CanOperateSorafsPopIssuer,
        CanSetSorafsPricing,
        CanSetSorafsReservePolicy,
        CanUpsertSorafsProviderCredit,
    );
}
mod soranet {
    use super::*;
    use iroha_executor_data_model::permission::soranet::{
        CanIngestSoranetPrivacy, CanIssueSoranetVpnQuote, CanManageSoranetVpnQuoteIssuers,
    };
    impl_owned_permission!(CanManageSoranetVpnQuoteIssuers, CanIngestSoranetPrivacy);
    fn validate_quote_issuer_delegation(
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        if context.curr_block.is_genesis()
            || CanManageSoranetVpnQuoteIssuers.is_owned_by(authority, host)
        {
            return Ok(());
        }
        Err(ValidationFail::NotPermitted(
            "CanManageSoranetVpnQuoteIssuers is required to grant or revoke VPN quote issuer authority"
                .to_owned(),
        ))
    }
    impl ValidateGrantRevoke for CanIssueSoranetVpnQuote {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            validate_quote_issuer_delegation(authority, context, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            validate_quote_issuer_delegation(authority, context, host)
        }
    }
}
mod oracle {
    use super::*;
    use iroha_executor_data_model::permission::oracle::{
        CanManageTwitterBindings, CanProposeOracleChange, CanRegisterOracleFeed,
        CanResolveOracleDispute, CanRollbackOracleChange, CanVoteOracleChangeStage,
    };
    impl_owned_permission!(
        CanRegisterOracleFeed,
        CanProposeOracleChange,
        CanVoteOracleChangeStage,
        CanRollbackOracleChange,
        CanResolveOracleDispute,
        CanManageTwitterBindings,
    );
}
mod peer {
    use super::*;
    use iroha_executor_data_model::permission::peer::{
        CanManageLaneRelayEmergency, CanManagePeers,
    };
    impl_validate_grant_revoke_via!(OnlyGenesis::from =>
        CanManagePeers,
        CanManageLaneRelayEmergency,
    );
}
mod role {
    use super::*;
    use iroha_executor_data_model::permission::role::CanManageRoles;
    impl_validate_grant_revoke_via!(OnlyGenesis::from => CanManageRoles);
}
mod parameter {
    //! Module with pass conditions for parameter related tokens
    use super::*;
    use iroha_executor_data_model::permission::parameter::CanSetParameters;
    impl ValidateGrantRevoke for CanSetParameters {
        fn validate_grant(
            &self,
            authority: &AccountId,
            _context: &Context,
            host: &Iroha,
        ) -> Result {
            if CanSetParameters.is_owned_by(authority, host) {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Current authority doesn't have the permission to set parameters, therefore it can't grant it to another account"
                    .to_owned()
            ))
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            _context: &Context,
            host: &Iroha,
        ) -> Result {
            if CanSetParameters.is_owned_by(authority, host) {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Current authority doesn't have the permission to set parameters, therefore it can't revoke it from another account"
                    .to_owned()
            ))
        }
    }
}
mod sccp {
    //! Pass conditions for governed SCCP state management.
    use super::*;
    use iroha_executor_data_model::permission::sccp::{
        CanManageSccpGovernance, CanProposeSccpRouteGovernance,
    };
    impl ValidateGrantRevoke for CanManageSccpGovernance {
        fn validate_grant(
            &self,
            authority: &AccountId,
            _context: &Context,
            host: &Iroha,
        ) -> Result {
            ensure_permission_owned(self, authority, host, "CanManageSccpGovernance")
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            _context: &Context,
            host: &Iroha,
        ) -> Result {
            ensure_permission_owned(self, authority, host, "CanManageSccpGovernance")
        }
    }
    impl ValidateGrantRevoke for CanProposeSccpRouteGovernance {
        fn validate_grant(
            &self,
            authority: &AccountId,
            _context: &Context,
            host: &Iroha,
        ) -> Result {
            if CanManageSccpGovernance.is_owned_by(authority, host) {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Only SCCP governance managers may grant CanProposeSccpRouteGovernance".to_owned(),
            ))
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            _context: &Context,
            host: &Iroha,
        ) -> Result {
            if CanManageSccpGovernance.is_owned_by(authority, host) {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Only SCCP governance managers may revoke CanProposeSccpRouteGovernance".to_owned(),
            ))
        }
    }
}
mod offline {
    //! Pass conditions for governed offline-settlement releases.
    use super::*;
    use iroha_executor_data_model::permission::offline::{
        CanActivateKagemushaRecursiveReleaseV4, CanManageOfflineDeviceAttestationPolicy,
        CanManageOfflineEscrow,
    };
    impl_validate_grant_revoke_via!(OnlyGenesis::from =>
        CanManageOfflineEscrow,
        CanActivateKagemushaRecursiveReleaseV4,
        CanManageOfflineDeviceAttestationPolicy,
    );
}
pub mod asset {
    //! Module with pass conditions for asset related tokens
    use super::*;
    use iroha_executor_data_model::permission::asset::{
        CanBurnAsset, CanBurnAssetWithDefinition, CanMintAssetToAccount,
        CanMintAssetWithDefinition, CanModifyAssetMetadata, CanModifyAssetMetadataWithDefinition,
        CanSetAssetHoldingLimit, CanSetAssetTransferAvailability, CanSetAssetTransferDailyLimit,
        CanTransferAsset, CanTransferAssetWithDefinition,
    };
    /// Check if `authority` is the owner of asset.
    ///
    /// `authority` is owner of asset if:
    /// - `asset_id.account_id` is `account_id`
    /// - `asset_id.account_id.domain_id` domain is owned by `authority`
    ///
    pub fn is_asset_owner(asset_id: &AssetId, authority: &AccountId, host: &Iroha) -> bool {
        crate::permission::account::is_account_owner(asset_id.account(), authority, host)
    }
    /// Pass condition that checks if `authority` is the owner of asset.
    #[derive(Debug, Clone)]
    pub struct Owner<'asset> {
        /// Asset id to check against
        pub asset: &'asset AssetId,
    }
    impl PassCondition for Owner<'_> {
        fn validate(&self, authority: &AccountId, host: &Iroha, _context: &Context) -> Result {
            if is_asset_owner(self.asset, authority, host) {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Can't access asset owned by another account".to_owned(),
            ))
        }
    }
    impl_validate_grant_revoke_via!(super::asset_definition::Owner::from =>
        CanMintAssetWithDefinition,
        CanBurnAssetWithDefinition,
        CanTransferAssetWithDefinition,
        CanModifyAssetMetadataWithDefinition,
        CanSetAssetTransferAvailability,
        CanSetAssetTransferDailyLimit,
        CanSetAssetHoldingLimit,
    );
    impl ValidateGrantRevoke for CanMintAssetToAccount {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            super::asset_definition::Owner {
                asset_definition: &self.asset_definition,
            }
            .validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            super::asset_definition::Owner {
                asset_definition: &self.asset_definition,
            }
            .validate(authority, host, context)
        }
    }
    impl ValidateGrantRevoke for CanBurnAsset {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            validate_asset_or_definition_owner(&self.asset, authority, context, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            validate_asset_or_definition_owner(&self.asset, authority, context, host)
        }
    }
    impl_validate_grant_revoke_via!(Owner::from => CanTransferAsset);
    macro_rules! impl_froms {
        ($($name:ty),+ $(,)?) => {$(
            impl<'t> From<&'t $name> for Owner<'t> {
                fn from(value: &'t $name) -> Self {
                    Self { asset: &value.asset}
                }
            })+
        };
    }
    impl_froms!(CanBurnAsset, CanTransferAsset, CanModifyAssetMetadata);
    impl ValidateGrantRevoke for CanModifyAssetMetadata {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            validate_asset_or_definition_owner(&self.asset, authority, context, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            validate_asset_or_definition_owner(&self.asset, authority, context, host)
        }
    }
    fn validate_asset_or_definition_owner(
        asset: &AssetId,
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        if is_asset_owner(asset, authority, host) {
            return Ok(());
        }
        super::asset_definition::Owner {
            asset_definition: asset.definition(),
        }
        .validate(authority, host, context)
    }
}
pub mod asset_definition {
    //! Module with pass conditions for asset definition related tokens
    use super::*;
    use crate::smart_contract::{
        Iroha,
        data_model::{isi::error::InstructionExecutionError, query::error::FindError},
    };
    use iroha_executor_data_model::permission::asset_definition::{
        AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias,
        CanManageAssetDefinitionConfidentialPolicy, CanModifyAssetDefinitionMetadata,
        CanUnregisterAssetDefinition,
    };
    /// Check if `authority` is the owner of asset definition
    ///
    /// `authority` is owner of asset definition if:
    /// - `asset_definition.owned_by` is `authority`
    ///
    /// # Errors
    /// - if `FindAssetDefinitionById` fails
    pub fn is_asset_definition_owner(
        asset_definition_id: &AssetDefinitionId,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result<bool> {
        let mut asset_definition_opt = None;
        let iter = host.query(FindAssetsDefinitions).execute()?;
        for item in iter {
            let ad = item.dbg_expect("Failed to get asset definition from cursor");
            if ad.id() == asset_definition_id {
                asset_definition_opt = Some(ad);
                break;
            }
        }
        let asset_definition = asset_definition_opt.ok_or_else(|| {
            ValidationFail::InstructionFailed(InstructionExecutionError::Find(
                FindError::AssetDefinition(asset_definition_id.clone()),
            ))
        })?;
        Ok(asset_definition.owned_by() == authority)
    }
    fn validate_asset_definition_alias_namespace_scope_owner(
        scope: &AssetDefinitionAliasPermissionScope,
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        match scope {
            AssetDefinitionAliasPermissionScope::Domain(domain) => {
                super::domain::Owner { domain }.validate(authority, host, context)
            }
            AssetDefinitionAliasPermissionScope::Dataspace(dataspace) => {
                super::account::validate_dataspace_alias_owner(*dataspace, authority, host)
            }
            AssetDefinitionAliasPermissionScope::Alias(_) => Err(ValidationFail::NotPermitted(
                "an exact asset-definition alias is not a namespace root".to_owned(),
            )),
        }
    }
    pub(super) fn asset_definition_alias_namespace_scope(
        alias: &ResolvedAssetDefinitionAliasV1,
    ) -> Result<AssetDefinitionAliasPermissionScope> {
        match alias.parent_domain() {
            Ok(Some(domain)) => Ok(AssetDefinitionAliasPermissionScope::Domain(domain)),
            Ok(None) => Ok(AssetDefinitionAliasPermissionScope::Dataspace(
                alias.dataspace_id,
            )),
            Err(error) => Err(ValidationFail::NotPermitted(format!(
                "Invalid exact asset-definition alias namespace `{alias}`: {error}"
            ))),
        }
    }
    fn validate_asset_definition_alias_namespace_authority(
        alias: &ResolvedAssetDefinitionAliasV1,
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        let scope = asset_definition_alias_namespace_scope(alias)?;
        let permission = CanManageAssetDefinitionAlias {
            scope: scope.clone(),
        };
        if permission.is_owned_by(authority, host) {
            return Ok(());
        }
        validate_asset_definition_alias_namespace_scope_owner(&scope, authority, context, host)
    }
    fn validate_active_asset_definition_alias_owner(
        alias: &ResolvedAssetDefinitionAliasV1,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result {
        let iter = host.query(FindAssetsDefinitions).execute()?;
        for item in iter {
            let definition = item.dbg_expect("Failed to get asset definition from cursor");
            if definition.id() == &alias.asset_definition_id
                && definition.alias().as_ref() == Some(&alias.canonical_name)
            {
                return if definition.owned_by() == authority {
                    Ok(())
                } else {
                    Err(ValidationFail::NotPermitted(format!(
                        "Only the owner of the definition bound to `{alias}` may grant its exact alias capability"
                    )))
                };
            }
        }
        Err(ValidationFail::NotPermitted(format!(
            "Asset-definition alias `{alias}` is not actively bound"
        )))
    }
    /// Pass condition that checks if `authority` is the owner of asset definition.
    #[derive(Debug, Clone)]
    pub struct Owner<'asset_definition> {
        /// Asset definition id to check against
        pub asset_definition: &'asset_definition AssetDefinitionId,
    }
    impl PassCondition for Owner<'_> {
        fn validate(&self, authority: &AccountId, host: &Iroha, _context: &Context) -> Result {
            if is_asset_definition_owner(self.asset_definition, authority, host)? {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Can't access asset definition owned by another account".to_owned(),
            ))
        }
    }
    impl_validate_grant_revoke_via!(Owner::from =>
        CanUnregisterAssetDefinition,
        CanModifyAssetDefinitionMetadata,
        CanManageAssetDefinitionConfidentialPolicy,
    );
    impl ValidateGrantRevoke for CanManageAssetDefinitionAlias {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            match &self.scope {
                AssetDefinitionAliasPermissionScope::Alias(alias) => {
                    validate_active_asset_definition_alias_owner(alias, authority, host)?;
                    validate_asset_definition_alias_namespace_authority(
                        alias, authority, context, host,
                    )
                }
                scope => validate_asset_definition_alias_namespace_scope_owner(
                    scope, authority, context, host,
                ),
            }
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            match &self.scope {
                AssetDefinitionAliasPermissionScope::Alias(alias) => {
                    let scope = asset_definition_alias_namespace_scope(alias)?;
                    validate_asset_definition_alias_namespace_scope_owner(
                        &scope, authority, context, host,
                    )
                }
                scope => validate_asset_definition_alias_namespace_scope_owner(
                    scope, authority, context, host,
                ),
            }
        }
    }
    macro_rules! impl_froms {
        ($($name:ty),+ $(,)?) => {$(
            impl<'t> From<&'t $name> for Owner<'t> {
                fn from(value: &'t $name) -> Self {
                    Self { asset_definition: &value.asset_definition }
                }
            })+
        };
    }
    impl_froms!(
        CanUnregisterAssetDefinition,
        CanModifyAssetDefinitionMetadata,
        CanManageAssetDefinitionConfidentialPolicy,
        iroha_executor_data_model::permission::asset::CanMintAssetWithDefinition,
        iroha_executor_data_model::permission::asset::CanBurnAssetWithDefinition,
        iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition,
        iroha_executor_data_model::permission::asset::CanModifyAssetMetadataWithDefinition,
        iroha_executor_data_model::permission::asset::CanSetAssetTransferAvailability,
        iroha_executor_data_model::permission::asset::CanSetAssetTransferDailyLimit,
        iroha_executor_data_model::permission::asset::CanSetAssetHoldingLimit,
    );
}
/// Module with pass conditions for NFT related tokens
///
/// - Owner of `nft.domain` can unregister, modify and transfer NFT
/// - Owner of NFT can only transfer NFT
///
/// So:
/// - *full* owner - can unregister, modify and transfer NFT
/// - *weak* owner - can transfer NFT
pub mod nft {
    use super::*;
    use crate::smart_contract::{
        Iroha,
        data_model::{isi::error::InstructionExecutionError, query::error::FindError},
    };
    use iroha_executor_data_model::permission::nft::{
        CanModifyNftMetadata, CanRegisterNft, CanTransferNft, CanUnregisterNft,
    };
    /// Check if `authority` is *week* owner of NFT.
    ///
    /// `authority` is *week* owner of NFT if:
    /// - `nft.owned_by` is `authority`
    /// - `nft.domain_id` domain is owned by `authority`
    ///
    /// Also see [nft] module documentation.
    ///
    /// # Errors
    /// - if `FindNfts` fails
    /// - if `is_domain_owner` fails
    pub fn is_nft_weak_owner(nft_id: &NftId, authority: &AccountId, host: &Iroha) -> Result<bool> {
        let mut nft_opt = None;
        let iter = host.query(FindNfts).execute()?;
        for item in iter {
            let nft = item.dbg_expect("Failed to get NFT from cursor");
            if nft.id() == nft_id {
                nft_opt = Some(nft);
                break;
            }
        }
        let nft = nft_opt.ok_or_else(|| {
            ValidationFail::InstructionFailed(InstructionExecutionError::Find(FindError::Nft(
                nft_id.clone(),
            )))
        })?;
        if nft.owned_by() == authority {
            Ok(true)
        } else {
            is_nft_full_owner(nft_id, authority, host)
        }
    }
    /// Check if `authority` is *full* owner of NFT.
    ///
    /// `authority` is *full* owner of NFT if:
    /// - `nft.domain_id` domain is owned by `authority`
    ///
    /// Also see [nft] module documentation.
    ///
    /// # Errors
    /// - if `is_domain_owner` fails
    pub fn is_nft_full_owner(nft_id: &NftId, authority: &AccountId, host: &Iroha) -> Result<bool> {
        domain::is_domain_owner(nft_id.domain(), authority, host)
    }
    /// Pass condition that checks if `authority` is the *weak* owner of NFT.
    #[derive(Debug, Clone)]
    pub struct WeakOwner<'nft> {
        /// NFT id to check against
        pub nft: &'nft NftId,
    }
    impl PassCondition for WeakOwner<'_> {
        fn validate(&self, authority: &AccountId, host: &Iroha, _context: &Context) -> Result {
            if is_nft_weak_owner(self.nft, authority, host)? {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Can't access NFT owned by another account".to_owned(),
            ))
        }
    }
    /// Pass condition that checks if `authority` is the *full* owner of NFT.
    #[derive(Debug, Clone)]
    pub struct FullOwner<'nft> {
        /// NFT id to check against
        pub nft: &'nft NftId,
    }
    impl PassCondition for FullOwner<'_> {
        fn validate(&self, authority: &AccountId, host: &Iroha, _context: &Context) -> Result {
            if is_nft_full_owner(self.nft, authority, host)? {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Can't access NFT from domain owned by another account".to_owned(),
            ))
        }
    }
    impl_validate_grant_revoke_via!(super::domain::Owner::from => CanRegisterNft);
    macro_rules! impl_froms_and_validate_grant_revoke {
        ($owner:ident : $($name:ty),+ $(,)?) => {$(
            impl<'t> From<&'t $name> for $owner<'t> {
                fn from(value: &'t $name) -> Self {
                    Self { nft: &value.nft }
                }
            }
            impl ValidateGrantRevoke for $name {
                fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
                    $owner::from(self).validate(authority, host, context)
                }
                fn validate_revoke(
                    &self,
                    authority: &AccountId,
                    context: &Context,
                    host: &Iroha,
                ) -> Result {
                    $owner::from(self).validate(authority, host, context)
                }
            }
        )+};
    }
    impl_froms_and_validate_grant_revoke!(WeakOwner: CanTransferNft);
    impl_froms_and_validate_grant_revoke!(FullOwner: CanUnregisterNft, CanModifyNftMetadata);
}
pub mod account {
    //! Module with pass conditions for asset related tokens
    use super::*;
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanDelegateAccountAliasResolution, CanManageAccountAlias,
        CanModifyAccountMetadata, CanRegisterAccount, CanReplaceAccountController,
        CanResolveAccountAlias, CanUnregisterAccount,
    };
    use iroha_executor_data_model::permission::query::CanReadAccountData;
    /// Check if `authority` is the owner of account.
    ///
    /// `authority` owns the account if it matches the account subject exactly.
    pub fn is_account_owner(account_id: &AccountId, authority: &AccountId, _host: &Iroha) -> bool {
        account_id == authority
    }
    /// Pass condition that checks if `authority` is the owner of account.
    #[derive(Debug, Clone)]
    pub struct Owner<'asset> {
        /// Account id to check against
        pub account: &'asset AccountId,
    }
    pub(super) fn validate_dataspace_alias_owner(
        dataspace: crate::smart_contract::data_model::nexus::DataSpaceId,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result {
        let owner = host
            .query_single(
                crate::smart_contract::data_model::query::sns::prelude::FindDataspaceNameOwnerById::new(dataspace),
            )
            .map_err(|_| {
                ValidationFail::NotPermitted(format!(
                    "Dataspace alias lease for `{dataspace}` has no active owner"
                ))
            })?;
        if &owner == authority {
            Ok(())
        } else {
            Err(ValidationFail::NotPermitted(format!(
                "Can't manage dataspace alias permissions for `{dataspace}`"
            )))
        }
    }
    fn validate_account_alias_domain_owner(
        domain: &crate::smart_contract::data_model::domain::DomainId,
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        super::domain::Owner { domain }.validate(authority, host, context)
    }
    fn validate_account_alias_scope_owner(
        scope: &AccountAliasPermissionScope,
        authority: &AccountId,
        context: &Context,
        host: &Iroha,
    ) -> Result {
        match scope {
            AccountAliasPermissionScope::Domain(domain) => {
                validate_account_alias_domain_owner(domain, authority, context, host)
            }
            AccountAliasPermissionScope::Dataspace(dataspace) => {
                validate_dataspace_alias_owner(*dataspace, authority, host)
            }
            AccountAliasPermissionScope::Alias(alias) => {
                let account = host
                    .query_single(
                        crate::smart_contract::data_model::query::account::prelude::FindAccountByAlias::new(
                            alias.account_alias(),
                        ),
                    )
                    .map_err(|_| {
                        ValidationFail::NotPermitted(format!(
                            "Account alias lease for `{alias}` has no active owner"
                        ))
                    })?;
                if account.id() == authority {
                    Ok(())
                } else {
                    Err(ValidationFail::NotPermitted(format!(
                        "Can't manage exact account alias permissions for `{alias}`"
                    )))
                }
            }
        }
    }
    fn can_delegate_account_alias_resolve(
        authority: &AccountId,
        scope: &AccountAliasPermissionScope,
        context: &Context,
        host: &Iroha,
    ) -> bool {
        let exact_delegation_permission = CanDelegateAccountAliasResolution {
            scope: scope.clone(),
        };
        let account_permissions_allow_delegation =
            exact_delegation_permission.is_owned_by(authority, host);
        account_permissions_allow_delegation
            || validate_account_alias_scope_owner(scope, authority, context, host).is_ok()
    }
    impl PassCondition for Owner<'_> {
        fn validate(&self, authority: &AccountId, host: &Iroha, _context: &Context) -> Result {
            if is_account_owner(self.account, authority, host) {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Can't access another account".to_owned(),
            ))
        }
    }
    impl_validate_grant_revoke_via!(super::domain::Owner::from => CanRegisterAccount);
    impl_validate_grant_revoke_via!(Owner::from =>
        CanUnregisterAccount,
        CanModifyAccountMetadata,
        CanReplaceAccountController,
        CanReadAccountData,
    );
    impl ValidateGrantRevoke for CanResolveAccountAlias {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            if can_delegate_account_alias_resolve(authority, &self.scope, context, host) {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Can't grant or revoke account-alias resolution outside an owned or exactly delegated scope"
                    .to_owned(),
            ))
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            self.validate_grant(authority, context, host)
        }
    }
    impl ValidateGrantRevoke for CanDelegateAccountAliasResolution {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            validate_account_alias_scope_owner(&self.scope, authority, context, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            validate_account_alias_scope_owner(&self.scope, authority, context, host)
        }
    }
    impl ValidateGrantRevoke for CanManageAccountAlias {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            validate_account_alias_scope_owner(&self.scope, authority, context, host)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            self.validate_grant(authority, context, host)
        }
    }
    macro_rules! impl_froms {
        ($($name:ty),+ $(,)?) => {$(
            impl<'t> From<&'t $name> for Owner<'t> {
                fn from(value: &'t $name) -> Self {
                    Self { account: &value.account }
                }
            })+
        };
    }
    impl_froms!(
        CanUnregisterAccount,
        CanModifyAccountMetadata,
        CanReplaceAccountController,
        CanReadAccountData,
    );
}
pub mod trigger {
    //! Module with pass conditions for trigger related tokens
    use super::*;
    use crate::data_model::{
        isi::error::InstructionExecutionError,
        query::{error::FindError, trigger::FindTriggers},
    };
    use iroha_executor_data_model::permission::trigger::{
        CanExecuteTrigger, CanModifyTrigger, CanModifyTriggerMetadata, CanRegisterTrigger,
        CanUnregisterTrigger,
    };
    /// Check if `authority` is the owner of trigger.
    ///
    /// `authority` owns the trigger if it matches the trigger authority exactly.
    ///
    /// # Errors
    /// - `FindTrigger` fails
    pub fn is_trigger_owner(
        trigger_id: &TriggerId,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result<bool> {
        let trigger = find_trigger(trigger_id, host)?;
        Ok(trigger.action().authority() == authority)
    }
    /// Returns the trigger.
    pub(crate) fn find_trigger(trigger_id: &TriggerId, host: &Iroha) -> Result<Trigger> {
        {
            let mut found = None;
            let iter = host.query(FindTriggers::new()).execute()?;
            for item in iter {
                let trg = item.dbg_expect("Failed to get trigger from cursor");
                if trg.id() == trigger_id {
                    found = Some(trg);
                    break;
                }
            }
            found.ok_or_else(|| {
                ValidationFail::InstructionFailed(InstructionExecutionError::Find(
                    FindError::Trigger(trigger_id.clone()),
                ))
            })
        }
    }
    /// Pass condition that checks if `authority` is the owner of trigger.
    #[derive(Debug, Clone)]
    pub struct Owner<'trigger> {
        /// Trigger id to check against
        pub trigger: &'trigger TriggerId,
    }
    impl PassCondition for Owner<'_> {
        fn validate(&self, authority: &AccountId, host: &Iroha, _context: &Context) -> Result {
            if is_trigger_owner(self.trigger, authority, host)? {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Can't give permission to access trigger owned by another account".to_owned(),
            ))
        }
    }
    impl_validate_grant_revoke_via!(super::account::Owner::from => CanRegisterTrigger);
    impl_validate_grant_revoke_via!(Owner::from =>
        CanExecuteTrigger,
        CanUnregisterTrigger,
        CanModifyTrigger,
        CanModifyTriggerMetadata,
    );
    impl<'t> From<&'t CanRegisterTrigger> for super::account::Owner<'t> {
        fn from(value: &'t CanRegisterTrigger) -> Self {
            Self {
                account: &value.authority,
            }
        }
    }
    macro_rules! impl_froms {
        ($($name:ty),+ $(,)?) => {$(
            impl<'t> From<&'t $name> for Owner<'t> {
                fn from(value: &'t $name) -> Self {
                    Self { trigger: &value.trigger }
                }
            })+
        };
    }
    impl_froms!(
        CanUnregisterTrigger,
        CanModifyTrigger,
        CanExecuteTrigger,
        CanModifyTriggerMetadata,
    );
}
pub mod domain {
    //! Module with pass conditions for domain related tokens
    use super::*;
    use iroha_executor_data_model::permission::{
        domain::{CanModifyDomainMetadata, CanRegisterDomain, CanUnregisterDomain},
        nft::CanRegisterNft,
    };
    use iroha_smart_contract::data_model::{
        isi::error::InstructionExecutionError, query::error::FindError,
    };
    /// Check if `authority` is owner of domain
    ///
    /// # Errors
    /// Fails if query fails
    pub fn is_domain_owner(
        domain_id: &DomainId,
        authority: &AccountId,
        host: &Iroha,
    ) -> Result<bool> {
        {
            let mut found = None;
            let iter = host.query(FindDomains).execute()?;
            for item in iter {
                let domain = item.dbg_expect("Failed to get domain from cursor");
                if domain.id() == domain_id {
                    found = Some(domain);
                    break;
                }
            }
            found.map(|d| d.owned_by() == authority).ok_or_else(|| {
                ValidationFail::InstructionFailed(InstructionExecutionError::Find(
                    FindError::Domain(domain_id.clone()),
                ))
            })
        }
    }
    /// Pass condition that checks if `authority` is the owner of domain.
    #[derive(Debug, Clone)]
    pub struct Owner<'domain> {
        /// Domain id to check against
        pub domain: &'domain DomainId,
    }
    impl PassCondition for Owner<'_> {
        fn validate(&self, authority: &AccountId, host: &Iroha, _context: &Context) -> Result {
            if is_domain_owner(self.domain, authority, host)? {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "Can't access domain owned by another account".to_owned(),
            ))
        }
    }
    impl_validate_grant_revoke_via!(OnlyGenesis::from => CanRegisterDomain);
    impl_validate_grant_revoke_via!(Owner::from =>
        CanUnregisterDomain,
        CanModifyDomainMetadata,
    );
    macro_rules! impl_froms {
        ($($name:ty),+ $(,)?) => {$(
            impl<'t> From<&'t $name> for Owner<'t> {
                fn from(value: &'t $name) -> Self {
                    Self { domain: &value.domain }
                }
            })+
        };
    }
    impl_froms!(
        CanUnregisterDomain,
        CanModifyDomainMetadata,
        iroha_executor_data_model::permission::account::CanRegisterAccount,
        CanRegisterNft,
    );
}
/// Pass condition that allows operation only in genesis.
///
/// In other words it always operation only if block height is 0.
#[derive(Debug, Default, Copy, Clone)]
pub(crate) struct OnlyGenesis;
impl PassCondition for OnlyGenesis {
    fn validate(&self, _authority: &AccountId, _host: &Iroha, context: &Context) -> Result {
        if context.curr_block.is_genesis() {
            Ok(())
        } else {
            Err(ValidationFail::NotPermitted(
                "This operation is only allowed inside the genesis block".to_owned(),
            ))
        }
    }
}
impl<T: Permission> From<&T> for OnlyGenesis {
    fn from(_: &T) -> Self {
        Self
    }
}
/// Iterator over all accounts and theirs permission tokens
pub(crate) fn accounts_permissions(
    host: &Iroha,
) -> impl Iterator<Item = (AccountId, PermissionObject)> + '_ {
    host.query(FindAccounts)
        .execute()
        .dbg_expect("INTERNAL BUG: `FindAllAccounts` must never fail")
        .map(|account| account.dbg_expect("Failed to get account from cursor"))
        .flat_map(|account| {
            host.query(FindPermissionsByAccountId::new(account.id().clone()))
                .execute()
                .dbg_expect("INTERNAL BUG: `FindPermissionsByAccountId` must never fail")
                .map(|permission| permission.dbg_expect("Failed to get permission from cursor"))
                .map(move |permission| (account.id().clone(), permission))
        })
}
/// Iterator over all roles and theirs permission tokens
pub(crate) fn roles_permissions(host: &Iroha) -> impl Iterator<Item = (RoleId, PermissionObject)> {
    host.query(FindRoles)
        .execute()
        .dbg_expect("INTERNAL BUG: `FindAllRoles` must never fail")
        .map(|role| role.dbg_expect("Failed to get role from cursor"))
        .flat_map(|role| {
            role.permissions()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .map(move |permission| (role.id().clone(), permission))
        })
}
fn has_permission_in_roles<P>(
    roles: impl IntoIterator<Item = (RoleId, PermissionObject)>,
    role_ids: &[RoleId],
    target: &P,
) -> bool
where
    P: Permission + PartialEq,
    for<'a> P: TryFrom<&'a PermissionObject>,
{
    if role_ids.is_empty() {
        return false;
    }
    let role_filter: BTreeSet<_> = role_ids.iter().cloned().collect();
    roles
        .into_iter()
        .filter(|(role_id, _)| role_filter.contains(role_id))
        .filter_map(|(_, permission)| P::try_from(&permission).ok())
        .any(|permission| *target == permission)
}
fn has_permission_in_account<P>(permissions: &[PermissionObject], target: &P) -> bool
where
    P: Permission + PartialEq,
    for<'a> P: TryFrom<&'a PermissionObject>,
{
    permissions
        .iter()
        .filter_map(|permission| P::try_from(permission).ok())
        .any(|permission| *target == permission)
}
pub(crate) fn permission_owned_in_sources<P>(
    account_permissions: &[PermissionObject],
    roles: &[(RoleId, PermissionObject)],
    role_ids: &[RoleId],
    target: &P,
) -> bool
where
    P: Permission + PartialEq,
    for<'a> P: TryFrom<&'a PermissionObject>,
{
    has_permission_in_account(account_permissions, target)
        || has_permission_in_roles(roles.iter().cloned(), role_ids, target)
}
/// Revoked all permissions satisfied given [condition].
///
/// Note: you must manually call `deny!` if this function returns error.
pub(crate) fn revoke_permissions<V: Execute + ?Sized>(
    executor: &mut V,
    condition: impl Fn(&PermissionObject) -> bool,
) -> Result<(), ValidationFail> {
    for (owner_id, permission) in accounts_permissions(executor.host()) {
        if condition(&permission) {
            let isi = RevokeBox::from(Revoke::account_permission(permission, owner_id.clone()));
            executor.host().submit(&isi)?;
        }
    }
    for (role_id, permission) in roles_permissions(executor.host()) {
        if condition(&permission) {
            let isi = RevokeBox::from(Revoke::role_permission(permission, role_id.clone()));
            executor.host().submit(&isi)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{
        AnyPermission, OnlyGenesis, PassCondition, ValidateGrantRevoke, has_permission_in_roles,
        permission_owned_in_sources,
    };
    use crate::permission::test_override;
    use crate::{
        data_model::ValidationFail,
        prelude::Context,
        smart_contract::{
            Iroha,
            data_model::{
                block::BlockHeader,
                nexus::{DataSpaceId, FeeSponsorProgramId, UniversalAccountId},
                permission::Permission as PermissionObject,
                prelude::{
                    AccountId, AssetDefinitionId, AssetId, DomainId, Json,
                    ResolvedAssetDefinitionAliasV1, RoleId,
                },
                smart_contract::ContractAddress,
            },
        },
    };
    use iroha_crypto::{Hash, PublicKey};
    use iroha_executor_data_model::permission::offline::{
        CanActivateKagemushaRecursiveReleaseV4, CanManageOfflineDeviceAttestationPolicy,
        CanManageOfflineEscrow,
    };
    use iroha_executor_data_model::permission::{
        account::{
            AccountAliasPermissionScope, CanDelegateAccountAliasResolution, CanResolveAccountAlias,
        },
        asset::{CanMintAssetToAccount, CanMintAssetWithDefinition},
        asset_definition::{
            AssetDefinitionAliasPermissionScope, CanManageAssetDefinitionAlias,
            CanManageAssetDefinitionConfidentialPolicy,
        },
        domain::CanRegisterDomain,
        governance::{
            CanManageConfidentialParams, CanManageConsensusKeys, CanManageRuntimeUpgrades,
        },
        nexus::{
            CanEnrollFeeSponsorProgram, CanManageFeeSponsorProgram,
            CanPublishSpaceDirectoryManifest, CanPublishSpaceDirectoryManifestForAccountDomain,
            CanPublishSpaceDirectoryManifestForUaid,
        },
        peer::CanManagePeers,
        query::{CanReadAccountData, CanReadAllLedgerData, CanReadRestrictedDataspace},
        settlement::CanExecuteSettlement,
        smart_contract::CanInvokeContractEntrypoint,
        soranet::{CanIssueSoranetVpnQuote, CanManageSoranetVpnQuoteIssuers},
    };
    use std::{num::NonZeroU64, vec::Vec};
    fn make_context(authority: &AccountId, height: u64) -> Context {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height must be non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        Context {
            authority: authority.clone(),
            curr_block: header,
        }
    }
    fn make_account_id() -> AccountId {
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        AccountId::new(public_key)
    }
    fn make_other_account_id() -> AccountId {
        let public_key: PublicKey =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .unwrap();
        AccountId::new(public_key)
    }
    #[test]
    fn operational_governance_permissions_require_canonical_unit_payloads() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let permissions = [
            PermissionObject::from(CanManageRuntimeUpgrades),
            PermissionObject::from(CanManageConsensusKeys),
            PermissionObject::from(CanManageConfidentialParams),
        ];

        for raw in permissions {
            let name = raw.name().to_owned();
            let dispatched =
                AnyPermission::try_from(&raw).expect("canonical unit permission must be typed");
            let previous = test_override::replace_permissions(vec![raw]);
            assert!(
                dispatched
                    .validate_grant(&authority, &context, &Iroha)
                    .is_ok(),
                "exact holder could not grant {name}",
            );
            assert!(
                dispatched
                    .validate_revoke(&authority, &context, &Iroha)
                    .is_ok(),
                "exact holder could not revoke {name}",
            );
            test_override::replace_permissions(previous);

            let malformed = PermissionObject::new(
                name.parse().expect("permission ident"),
                Json::from_raw_json("{\"invented_scope\":true}".to_owned())
                    .expect("valid JSON fixture"),
            );
            assert!(
                AnyPermission::try_from(&malformed).is_err(),
                "same-name non-unit {name} payload must fail typed dispatch",
            );
        }
    }
    fn make_third_account_id() -> AccountId {
        let public_key: PublicKey =
            "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016"
                .parse()
                .unwrap();
        AccountId::new(public_key)
    }
    fn make_fee_sponsor_program_id(sponsor: AccountId, name: &str) -> FeeSponsorProgramId {
        FeeSponsorProgramId::new(
            sponsor,
            name.parse()
                .expect("fee sponsor program name must be valid"),
        )
    }
    #[test]
    fn has_permission_in_roles_filters_by_role_ids() {
        let role_id: RoleId = "role1".parse().unwrap();
        let other_role_id: RoleId = "role2".parse().unwrap();
        let permission = PermissionObject::from(CanManagePeers);
        let roles = vec![
            (role_id.clone(), permission.clone()),
            (other_role_id, PermissionObject::from(CanManagePeers)),
        ];
        let role_ids = vec![role_id];
        assert!(has_permission_in_roles(roles, &role_ids, &CanManagePeers));
    }
    #[test]
    fn has_permission_in_roles_returns_false_when_role_ids_empty() {
        let role_id: RoleId = "role1".parse().unwrap();
        let roles = vec![(role_id, PermissionObject::from(CanManagePeers))];
        let role_ids: Vec<RoleId> = Vec::new();
        assert!(!has_permission_in_roles(roles, &role_ids, &CanManagePeers));
    }
    #[test]
    fn has_permission_in_roles_deduplicates_role_ids() {
        let role_id: RoleId = "role1".parse().unwrap();
        let roles = vec![(role_id.clone(), PermissionObject::from(CanManagePeers))];
        let role_ids = vec![role_id.clone(), role_id];
        assert!(has_permission_in_roles(roles, &role_ids, &CanManagePeers));
    }
    #[test]
    fn permission_owned_returns_true_for_direct_permission() {
        let account_permissions = vec![PermissionObject::from(CanManagePeers)];
        let role_permissions: Vec<(RoleId, PermissionObject)> = Vec::new();
        let role_ids: Vec<RoleId> = Vec::new();
        assert!(permission_owned_in_sources(
            &account_permissions,
            &role_permissions,
            &role_ids,
            &CanManagePeers,
        ));
    }
    #[test]
    fn permission_owned_returns_true_via_roles() {
        let role_id: RoleId = "validators".parse().unwrap();
        let account_permissions = Vec::new();
        let role_permissions = vec![(role_id.clone(), PermissionObject::from(CanManagePeers))];
        let role_ids = vec![role_id];
        assert!(permission_owned_in_sources(
            &account_permissions,
            &role_permissions,
            &role_ids,
            &CanManagePeers,
        ));
    }
    #[test]
    fn permission_owned_returns_false_when_missing() {
        let account_permissions = vec![PermissionObject::from(CanManagePeers)];
        let role_permissions: Vec<(RoleId, PermissionObject)> = Vec::new();
        let role_ids: Vec<RoleId> = Vec::new();
        assert!(!permission_owned_in_sources(
            &account_permissions,
            &role_permissions,
            &role_ids,
            &CanRegisterDomain,
        ));
    }
    #[test]
    fn permission_owned_matches_opaque_asset_definition_permission() {
        let asset_definition = AssetDefinitionId::from_uuid_bytes([
            0x68, 0x72, 0x45, 0x4e, 0x9c, 0x04, 0x46, 0x41, 0xaa, 0x58, 0x1e, 0xc5, 0xf3, 0x80,
            0x16, 0x19,
        ])
        .expect("opaque asset definition parses");
        let token = CanMintAssetWithDefinition {
            asset_definition: asset_definition.clone(),
        };
        let account_permissions = vec![PermissionObject::from(token.clone())];
        let role_permissions: Vec<(RoleId, PermissionObject)> = Vec::new();
        let role_ids: Vec<RoleId> = Vec::new();
        assert!(permission_owned_in_sources(
            &account_permissions,
            &role_permissions,
            &role_ids,
            &token,
        ));
    }
    #[test]
    fn confidential_policy_permission_holder_can_delegate_only_the_exact_asset_definition() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let target = AssetDefinitionId::derive_from_components(
            DomainId::try_new("currency", "paynet").expect("asset domain"),
            "pkr".parse().expect("asset name"),
        );
        let other = AssetDefinitionId::derive_from_components(
            DomainId::try_new("currency", "paynet").expect("asset domain"),
            "usd".parse().expect("asset name"),
        );
        let exact = CanManageAssetDefinitionConfidentialPolicy {
            asset_definition: target,
        };
        let sibling = CanManageAssetDefinitionConfidentialPolicy {
            asset_definition: other,
        };
        let held: PermissionObject = exact.clone().into();
        let exact_dispatched =
            AnyPermission::try_from(&held).expect("confidential-policy permission must be typed");
        let sibling_dispatched = AnyPermission::try_from(&PermissionObject::from(sibling))
            .expect("sibling confidential-policy permission must be typed");
        let previous = test_override::replace_permissions(vec![held]);
        let exact_grant = exact_dispatched.validate_grant(&authority, &context, &Iroha);
        let exact_revoke = exact_dispatched.validate_revoke(&authority, &context, &Iroha);
        let sibling_grant = sibling_dispatched.validate_grant(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(exact_grant.is_ok());
        assert!(exact_revoke.is_ok());
        assert!(matches!(
            sibling_grant,
            Err(ValidationFail::NotPermitted(_))
        ));
    }
    #[test]
    fn only_genesis_allows_first_block() {
        let authority = make_account_id();
        let context = make_context(&authority, 1);
        assert!(OnlyGenesis.validate(&authority, &Iroha, &context).is_ok());
    }
    #[test]
    fn only_genesis_rejects_other_blocks() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let err = OnlyGenesis
            .validate(&authority, &Iroha, &context)
            .expect_err("expected rejection");
        assert!(matches!(err, ValidationFail::NotPermitted(_)));
    }
    #[test]
    fn bilateral_settlement_consent_is_controlled_by_debited_account() {
        let debited_account = make_account_id();
        let other = make_other_account_id();
        let permission = CanExecuteSettlement {
            debited_asset: AssetId::new(
                AssetDefinitionId::derive_from_components(
                    DomainId::try_new("fixture", "universal").expect("asset domain"),
                    "rose".parse().expect("asset name"),
                ),
                debited_account.clone(),
            ),
            settlement_id: "permission_consent".parse().expect("settlement id"),
            intent_hash: Hash::new(b"exact bilateral settlement intent"),
        };
        let debited_context = make_context(&debited_account, 2);
        let other_context = make_context(&other, 2);
        permission
            .validate_grant(&debited_account, &debited_context, &Iroha)
            .expect("debited account may grant exact consent");
        permission
            .validate_revoke(&debited_account, &debited_context, &Iroha)
            .expect("debited account may revoke exact consent");
        assert!(matches!(
            permission
                .validate_grant(&other, &other_context, &Iroha)
                .expect_err("unrelated authority must not grant consent"),
            ValidationFail::NotPermitted(_)
        ));
    }
    #[test]
    fn vpn_quote_issuer_leaf_requires_manager_delegation() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let leaf = AnyPermission::CanIssueSoranetVpnQuote(CanIssueSoranetVpnQuote);
        let previous = test_override::replace_permissions(Vec::new());
        assert!(matches!(
            leaf.validate_grant(&authority, &context, &Iroha)
                .expect_err("an unrelated account must not appoint a VPN quote issuer"),
            ValidationFail::NotPermitted(_)
        ));
        test_override::replace_permissions(vec![CanIssueSoranetVpnQuote.into()]);
        assert!(matches!(
            leaf.validate_grant(&authority, &context, &Iroha)
                .expect_err("an issuer leaf must not propagate itself"),
            ValidationFail::NotPermitted(_)
        ));
        test_override::replace_permissions(vec![CanManageSoranetVpnQuoteIssuers.into()]);
        leaf.validate_grant(&authority, &context, &Iroha)
            .expect("the issuer manager may grant the leaf");
        leaf.validate_revoke(&authority, &context, &Iroha)
            .expect("the issuer manager may revoke the leaf");
        test_override::replace_permissions(previous);
    }
    #[test]
    fn governed_offline_permissions_are_immutable_after_genesis() {
        let banking_authority = make_account_id();
        let context = make_context(&banking_authority, 2);
        let results = [
            (
                "CanManageOfflineEscrow",
                CanManageOfflineEscrow.validate_grant(&banking_authority, &context, &Iroha),
                CanManageOfflineEscrow.validate_revoke(&banking_authority, &context, &Iroha),
            ),
            (
                "CanActivateKagemushaRecursiveReleaseV4",
                CanActivateKagemushaRecursiveReleaseV4.validate_grant(
                    &banking_authority,
                    &context,
                    &Iroha,
                ),
                CanActivateKagemushaRecursiveReleaseV4.validate_revoke(
                    &banking_authority,
                    &context,
                    &Iroha,
                ),
            ),
            (
                "CanManageOfflineDeviceAttestationPolicy",
                CanManageOfflineDeviceAttestationPolicy.validate_grant(
                    &banking_authority,
                    &context,
                    &Iroha,
                ),
                CanManageOfflineDeviceAttestationPolicy.validate_revoke(
                    &banking_authority,
                    &context,
                    &Iroha,
                ),
            ),
        ];
        for (name, grant, revoke) in results {
            for result in [grant, revoke] {
                let error = result.expect_err(
                    "a genesis-seeded offline permission must not be mutated post-genesis",
                );
                assert!(matches!(error, ValidationFail::NotPermitted(_)));
                assert!(
                    error
                        .to_string()
                        .contains("only allowed inside the genesis block"),
                    "unexpected {name} mutation rejection: {error}",
                );
            }
        }
    }
    #[test]
    fn governed_offline_permissions_can_only_be_seeded_in_genesis() {
        let genesis_authority = make_account_id();
        let context = make_context(&genesis_authority, 1);
        let results = [
            (
                "CanManageOfflineEscrow",
                CanManageOfflineEscrow.validate_grant(&genesis_authority, &context, &Iroha),
                CanManageOfflineEscrow.validate_revoke(&genesis_authority, &context, &Iroha),
            ),
            (
                "CanActivateKagemushaRecursiveReleaseV4",
                CanActivateKagemushaRecursiveReleaseV4.validate_grant(
                    &genesis_authority,
                    &context,
                    &Iroha,
                ),
                CanActivateKagemushaRecursiveReleaseV4.validate_revoke(
                    &genesis_authority,
                    &context,
                    &Iroha,
                ),
            ),
            (
                "CanManageOfflineDeviceAttestationPolicy",
                CanManageOfflineDeviceAttestationPolicy.validate_grant(
                    &genesis_authority,
                    &context,
                    &Iroha,
                ),
                CanManageOfflineDeviceAttestationPolicy.validate_revoke(
                    &genesis_authority,
                    &context,
                    &Iroha,
                ),
            ),
        ];
        for (name, grant, revoke) in results {
            assert!(grant.is_ok(), "genesis must grant {name}: {grant:?}");
            assert!(revoke.is_ok(), "genesis must revoke {name}: {revoke:?}");
        }
    }
    #[test]
    fn fee_sponsor_program_manager_is_typed_and_only_the_sponsor_may_delegate_it() {
        let sponsor = make_account_id();
        let outsider = make_other_account_id();
        let context = make_context(&sponsor, 2);
        let permission = CanManageFeeSponsorProgram {
            sponsor: sponsor.clone(),
        };
        let raw: PermissionObject = permission.clone().into();
        assert!(matches!(
            AnyPermission::try_from(&raw),
            Ok(AnyPermission::CanManageFeeSponsorProgram(parsed)) if parsed == permission
        ));
        assert!(
            permission
                .validate_grant(&sponsor, &context, &Iroha)
                .is_ok()
        );
        assert!(
            permission
                .validate_revoke(&sponsor, &context, &Iroha)
                .is_ok()
        );
        assert!(matches!(
            permission.validate_grant(&outsider, &context, &Iroha),
            Err(ValidationFail::NotPermitted(_))
        ));
        assert!(matches!(
            permission.validate_revoke(&outsider, &context, &Iroha),
            Err(ValidationFail::NotPermitted(_))
        ));
    }
    #[test]
    fn alias_resolution_delegate_can_propagate_only_the_exact_scope() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let delegated_scope = AccountAliasPermissionScope::Dataspace(DataSpaceId::new(10));
        let other_scope = AccountAliasPermissionScope::Dataspace(DataSpaceId::new(12));
        let held = CanDelegateAccountAliasResolution {
            scope: delegated_scope.clone(),
        };
        let exact = CanResolveAccountAlias {
            scope: delegated_scope,
        };
        let other = CanResolveAccountAlias { scope: other_scope };
        let held_object = PermissionObject::from(held.clone());
        let held_dispatched =
            AnyPermission::try_from(&held_object).expect("delegation token must be typed");
        let previous = test_override::replace_permissions(vec![held_object]);
        let exact_grant = exact.validate_grant(&authority, &context, &Iroha);
        let exact_revoke = exact.validate_revoke(&authority, &context, &Iroha);
        let cross_scope_grant = other.validate_grant(&authority, &context, &Iroha);
        let recursive_grant = held_dispatched.validate_grant(&authority, &context, &Iroha);
        let recursive_revoke = held_dispatched.validate_revoke(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(exact_grant.is_ok());
        assert!(exact_revoke.is_ok());
        assert!(matches!(
            cross_scope_grant,
            Err(ValidationFail::NotPermitted(_))
        ));
        assert!(recursive_grant.is_ok());
        assert!(recursive_revoke.is_ok());
    }
    #[test]
    fn exact_asset_alias_holder_cannot_revoke_after_binding_clear_without_namespace_root() {
        let holder = make_account_id();
        let context = make_context(&holder, 2);
        let target = ResolvedAssetDefinitionAliasV1::new(
            "usd#banka.paynet".parse().expect("asset alias"),
            DataSpaceId::new(7),
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("banka", "paynet").expect("alias domain"),
                "usd".parse().expect("asset name"),
            ),
        );
        let exact = CanManageAssetDefinitionAlias {
            scope: AssetDefinitionAliasPermissionScope::Alias(target),
        };
        let exact_raw = PermissionObject::from(exact.clone());
        let dispatched =
            AnyPermission::try_from(&exact_raw).expect("exact alias permission must be typed");
        // The binding is intentionally absent. Exact possession must not bypass the native
        // namespace-root lookup; the definition pin prevents rebinding escalation, while the
        // namespace root remains the lifecycle authority after clear.
        let previous = test_override::replace_permissions(vec![exact_raw]);
        let holder_revoke = dispatched.validate_revoke(&holder, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(holder_revoke.is_err());
        assert!(matches!(
            super::asset_definition::asset_definition_alias_namespace_scope(match &exact.scope {
                AssetDefinitionAliasPermissionScope::Alias(alias) => alias,
                _ => unreachable!("test constructs an exact alias"),
            }).expect("valid exact alias namespace"),
            AssetDefinitionAliasPermissionScope::Domain(domain)
                if domain == DomainId::try_new("banka", "paynet").expect("alias domain")
        ));
    }
    #[test]
    fn exact_holder_dispatch_covers_each_corrected_delegation_family() {
        let authority = make_account_id();
        let adjacent_owner = make_other_account_id();
        let context = make_context(&authority, 2);
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("grant_policy", "universal").expect("asset domain"),
            "root_asset".parse().expect("asset name"),
        );
        let contract = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &adjacent_owner,
            77,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let dataspace = DataSpaceId::new(7);
        let permissions = vec![
            PermissionObject::from(CanMintAssetToAccount {
                // Possessing this exact token authorizes propagation even though the authority is
                // neither the destination account nor queried as the definition owner.
                asset_definition,
                account: adjacent_owner,
            }),
            PermissionObject::from(CanInvokeContractEntrypoint {
                contract,
                entrypoint: "main".to_owned(),
            }),
            PermissionObject::from(CanDelegateAccountAliasResolution {
                scope: AccountAliasPermissionScope::Dataspace(dataspace),
            }),
            PermissionObject::from(CanPublishSpaceDirectoryManifestForUaid {
                dataspace,
                uaid: UniversalAccountId::from_hash(Hash::new(b"grant-policy-uaid")),
            }),
            PermissionObject::from(CanPublishSpaceDirectoryManifestForAccountDomain {
                dataspace,
                domain: DomainId::try_new("retail", "universal").expect("account domain"),
            }),
            PermissionObject::from(CanEnrollFeeSponsorProgram {
                program_id: make_fee_sponsor_program_id(authority.clone(), "retail"),
            }),
        ];
        for raw in permissions {
            let name = raw.name().to_owned();
            let dispatched =
                AnyPermission::try_from(&raw).expect("corrected permission must be typed");
            let previous = test_override::replace_permissions(vec![raw]);
            let grant = dispatched.validate_grant(&authority, &context, &Iroha);
            let revoke = dispatched.validate_revoke(&authority, &context, &Iroha);
            test_override::replace_permissions(previous);
            assert!(
                grant.is_ok(),
                "exact holder could not grant {name}: {grant:?}"
            );
            assert!(
                revoke.is_ok(),
                "exact holder could not revoke {name}: {revoke:?}",
            );
        }
    }
    #[test]
    fn exact_contract_holder_cannot_propagate_noncanonical_selector() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let raw = PermissionObject::from(CanInvokeContractEntrypoint {
            contract: ContractAddress::derive(
                &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                    .parse()
                    .expect("canonical test network id"),
                &make_other_account_id(),
                88,
                DataSpaceId::UNIVERSAL,
            )
            .expect("contract address"),
            entrypoint: " main".to_owned(),
        });
        let dispatched =
            AnyPermission::try_from(&raw).expect("contract permission must be structurally typed");
        let previous = test_override::replace_permissions(vec![raw]);
        let grant = dispatched.validate_grant(&authority, &context, &Iroha);
        let revoke = dispatched.validate_revoke(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        for result in [grant, revoke] {
            assert!(matches!(result, Err(ValidationFail::NotPermitted(_))));
        }
    }
    #[test]
    fn restricted_dataspace_reader_cannot_grant_or_revoke_after_genesis() {
        let authority = make_account_id();
        let post_genesis = make_context(&authority, 2);
        let exact = CanReadRestrictedDataspace {
            dataspace: DataSpaceId::new(10),
        };
        let permission = PermissionObject::from(exact);
        let role_dispatched =
            AnyPermission::try_from(&permission).expect("restricted-read permission must be typed");
        let previous = test_override::replace_permissions(vec![permission]);
        let denied = [
            exact.validate_grant(&authority, &post_genesis, &Iroha),
            exact.validate_revoke(&authority, &post_genesis, &Iroha),
            role_dispatched.validate_grant(&authority, &post_genesis, &Iroha),
            role_dispatched.validate_revoke(&authority, &post_genesis, &Iroha),
        ];
        test_override::replace_permissions(previous);
        for result in denied {
            let error = result
                .expect_err("a restricted reader must not mutate the exact token after genesis");
            assert!(matches!(error, ValidationFail::NotPermitted(_)));
            assert!(
                error
                    .to_string()
                    .contains("only allowed inside the genesis block"),
                "unexpected restricted-read mutation rejection: {error}",
            );
        }
        let genesis = make_context(&authority, 1);
        assert!(exact.validate_grant(&authority, &genesis, &Iroha).is_ok());
        assert!(exact.validate_revoke(&authority, &genesis, &Iroha).is_ok());
    }
    #[test]
    fn global_ledger_reader_cannot_grant_or_revoke_after_genesis() {
        let authority = make_account_id();
        let post_genesis = make_context(&authority, 2);
        let exact = CanReadAllLedgerData;
        let permission = PermissionObject::from(exact);
        let dispatched =
            AnyPermission::try_from(&permission).expect("global-read permission must be typed");
        let previous = test_override::replace_permissions(vec![permission]);
        let denied = [
            dispatched.validate_grant(&authority, &post_genesis, &Iroha),
            dispatched.validate_revoke(&authority, &post_genesis, &Iroha),
        ];
        test_override::replace_permissions(previous);
        for result in denied {
            let error = result.expect_err(
                "possession of the global read root must not permit post-genesis propagation",
            );
            assert!(matches!(error, ValidationFail::NotPermitted(_)));
        }
        let genesis = make_context(&authority, 1);
        assert!(exact.validate_grant(&authority, &genesis, &Iroha).is_ok());
        assert!(exact.validate_revoke(&authority, &genesis, &Iroha).is_ok());
    }
    #[test]
    fn account_subject_exclusively_controls_account_read_grants() {
        let account = make_account_id();
        let reader = make_other_account_id();
        let context = make_context(&account, 2);
        let exact = CanReadAccountData {
            account: account.clone(),
        };
        let permission = PermissionObject::from(exact.clone());
        let dispatched =
            AnyPermission::try_from(&permission).expect("account-read permission must be typed");
        assert!(
            dispatched
                .validate_grant(&account, &context, &Iroha)
                .is_ok(),
            "the account subject must control its read grant"
        );
        assert!(
            dispatched
                .validate_revoke(&account, &context, &Iroha)
                .is_ok(),
            "the account subject must control revocation"
        );
        let previous = test_override::replace_permissions(vec![permission]);
        let reader_context = make_context(&reader, 2);
        let denied = [
            dispatched.validate_grant(&reader, &reader_context, &Iroha),
            dispatched.validate_revoke(&reader, &reader_context, &Iroha),
        ];
        test_override::replace_permissions(previous);
        for result in denied {
            assert!(
                matches!(result, Err(ValidationFail::NotPermitted(_))),
                "an exact reader may use but must not propagate the account's grant"
            );
        }
    }
    #[test]
    fn alias_resolution_domain_delegate_cannot_widen_or_cross_scope_kind() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let delegated_scope = AccountAliasPermissionScope::Domain(
            iroha_data_model::domain::DomainId::try_new("hbl", "sbp")
                .expect("HBL SBP domain fixture"),
        );
        let sibling_scope = AccountAliasPermissionScope::Domain(
            iroha_data_model::domain::DomainId::try_new("ubl", "sbp")
                .expect("UBL SBP domain fixture"),
        );
        let held = CanDelegateAccountAliasResolution {
            scope: delegated_scope.clone(),
        };
        let exact = CanResolveAccountAlias {
            scope: delegated_scope,
        };
        let sibling = CanResolveAccountAlias {
            scope: sibling_scope,
        };
        let dataspace = CanResolveAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::new(10)),
        };
        let previous = test_override::replace_permissions(vec![PermissionObject::from(held)]);
        let exact_grant = exact.validate_grant(&authority, &context, &Iroha);
        let sibling_grant = sibling.validate_grant(&authority, &context, &Iroha);
        let dataspace_grant = dataspace.validate_grant(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(exact_grant.is_ok());
        assert!(matches!(
            sibling_grant,
            Err(ValidationFail::NotPermitted(_))
        ));
        assert!(matches!(
            dataspace_grant,
            Err(ValidationFail::NotPermitted(_))
        ));
    }
    #[test]
    fn can_publish_space_directory_manifest_grant_allows_existing_holder_after_genesis() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let token = CanPublishSpaceDirectoryManifest {
            dataspace: DataSpaceId::new(10),
        };
        let previous = test_override::replace_permissions(vec![PermissionObject::from(token)]);
        let result = token.validate_grant(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(result.is_ok());
    }
    #[test]
    fn can_publish_space_directory_manifest_grant_rejects_unscoped_null_payload_after_genesis() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let token = CanPublishSpaceDirectoryManifest {
            dataspace: DataSpaceId::new(10),
        };
        let previous = test_override::replace_permissions(vec![PermissionObject::new(
            "CanPublishSpaceDirectoryManifest"
                .parse()
                .expect("permission ident"),
            Json::from_raw_json("null".to_owned()).expect("valid JSON fixture"),
        )]);
        let result = token.validate_grant(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(matches!(result, Err(ValidationFail::NotPermitted(_))));
    }
    #[test]
    fn can_publish_space_directory_manifest_grant_rejects_missing_holder_after_genesis() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let token = CanPublishSpaceDirectoryManifest {
            dataspace: DataSpaceId::new(10),
        };
        let previous =
            test_override::replace_permissions(vec![PermissionObject::from(CanManagePeers)]);
        let err = token
            .validate_grant(&authority, &context, &Iroha)
            .expect_err("expected rejection");
        test_override::replace_permissions(previous);
        assert!(matches!(err, ValidationFail::NotPermitted(_)));
    }
    #[test]
    fn dataspace_manifest_holder_can_delegate_one_exact_uaid_after_genesis() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let dataspace = DataSpaceId::new(10);
        let scoped = CanPublishSpaceDirectoryManifestForUaid {
            dataspace,
            uaid: UniversalAccountId::from_hash(Hash::new(b"uaid::delegated-customer")),
        };
        let previous = test_override::replace_permissions(vec![PermissionObject::from(
            CanPublishSpaceDirectoryManifest { dataspace },
        )]);
        let result = scoped.validate_grant(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(result.is_ok());
    }
    #[test]
    fn uaid_manifest_holder_cannot_delegate_a_different_uaid_after_genesis() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let dataspace = DataSpaceId::new(10);
        let held = CanPublishSpaceDirectoryManifestForUaid {
            dataspace,
            uaid: UniversalAccountId::from_hash(Hash::new(b"uaid::hbl-customer")),
        };
        let requested = CanPublishSpaceDirectoryManifestForUaid {
            dataspace,
            uaid: UniversalAccountId::from_hash(Hash::new(b"uaid::ubl-customer")),
        };
        let previous = test_override::replace_permissions(vec![PermissionObject::from(held)]);
        let result = requested.validate_grant(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(matches!(result, Err(ValidationFail::NotPermitted(_))));
    }
    #[test]
    fn account_domain_manifest_delegation_is_exact_across_hbl_and_ubl() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let dataspace = DataSpaceId::new(10);
        let hbl = CanPublishSpaceDirectoryManifestForAccountDomain {
            dataspace,
            domain: DomainId::try_new("hbl", "sbp").expect("HBL domain"),
        };
        let ubl = CanPublishSpaceDirectoryManifestForAccountDomain {
            dataspace,
            domain: DomainId::try_new("ubl", "sbp").expect("UBL domain"),
        };
        let previous =
            test_override::replace_permissions(vec![PermissionObject::from(hbl.clone())]);
        let own_result = hbl.validate_grant(&authority, &context, &Iroha);
        let cross_fi_result = ubl.validate_grant(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(own_result.is_ok());
        assert!(matches!(
            cross_fi_result,
            Err(ValidationFail::NotPermitted(_))
        ));
    }
    #[test]
    fn dataspace_manifest_holder_can_delegate_account_domain_scope() {
        let authority = make_account_id();
        let context = make_context(&authority, 2);
        let dataspace = DataSpaceId::new(10);
        let hbl = CanPublishSpaceDirectoryManifestForAccountDomain {
            dataspace,
            domain: DomainId::try_new("hbl", "sbp").expect("HBL domain"),
        };
        let previous = test_override::replace_permissions(vec![PermissionObject::from(
            CanPublishSpaceDirectoryManifest { dataspace },
        )]);
        let result = hbl.validate_grant(&authority, &context, &Iroha);
        test_override::replace_permissions(previous);
        assert!(result.is_ok());
    }
    #[test]
    fn fee_program_manager_can_delegate_exact_enrollment_scope() {
        let sponsor = make_account_id();
        let manager = make_other_account_id();
        let context = make_context(&manager, 2);
        let program_id = make_fee_sponsor_program_id(sponsor.clone(), "retail");
        let enrollment = CanEnrollFeeSponsorProgram {
            program_id: program_id.clone(),
        };
        let previous = test_override::replace_permissions(vec![PermissionObject::from(
            CanManageFeeSponsorProgram { sponsor },
        )]);
        for result in [
            enrollment.validate_grant(&manager, &context, &Iroha),
            enrollment.validate_revoke(&manager, &context, &Iroha),
        ] {
            assert!(result.is_ok(), "program manager must delegate exact scopes");
        }
        test_override::replace_permissions(previous);
    }
    #[test]
    fn fee_program_delegation_is_exact_to_the_program_sponsor() {
        let first_sponsor = make_account_id();
        let second_sponsor = make_third_account_id();
        let manager = make_other_account_id();
        let context = make_context(&manager, 2);
        let first = CanEnrollFeeSponsorProgram {
            program_id: make_fee_sponsor_program_id(first_sponsor.clone(), "retail"),
        };
        let second = CanEnrollFeeSponsorProgram {
            program_id: make_fee_sponsor_program_id(second_sponsor, "retail"),
        };
        let previous = test_override::replace_permissions(vec![PermissionObject::from(
            CanManageFeeSponsorProgram {
                sponsor: first_sponsor,
            },
        )]);
        assert!(first.validate_grant(&manager, &context, &Iroha).is_ok());
        assert!(matches!(
            second.validate_grant(&manager, &context, &Iroha),
            Err(ValidationFail::NotPermitted(_))
        ));
        test_override::replace_permissions(previous);
    }
    #[test]
    fn exact_fee_program_enrollment_holder_can_propagate_exact_token() {
        let sponsor = make_account_id();
        let registrar = make_other_account_id();
        let context = make_context(&registrar, 2);
        let token = CanEnrollFeeSponsorProgram {
            program_id: make_fee_sponsor_program_id(sponsor, "retail"),
        };
        let raw = PermissionObject::from(token);
        let dispatched =
            AnyPermission::try_from(&raw).expect("fee-program enrollment token must be typed");
        let previous = test_override::replace_permissions(vec![raw]);
        assert!(
            dispatched
                .validate_grant(&registrar, &context, &Iroha)
                .is_ok()
        );
        assert!(
            dispatched
                .validate_revoke(&registrar, &context, &Iroha)
                .is_ok()
        );
        test_override::replace_permissions(previous);
    }
    #[test]
    fn genesis_can_seed_fee_program_permissions() {
        let sponsor = make_account_id();
        let genesis_authority = make_other_account_id();
        let context = make_context(&genesis_authority, 1);
        let program_id = make_fee_sponsor_program_id(sponsor.clone(), "retail");
        let permissions = [
            AnyPermission::CanManageFeeSponsorProgram(CanManageFeeSponsorProgram { sponsor }),
            AnyPermission::CanEnrollFeeSponsorProgram(CanEnrollFeeSponsorProgram { program_id }),
        ];
        for permission in permissions {
            assert!(
                permission
                    .validate_grant(&genesis_authority, &context, &Iroha)
                    .is_ok()
            );
            assert!(
                permission
                    .validate_revoke(&genesis_authority, &context, &Iroha)
                    .is_ok()
            );
        }
    }
    #[test]
    fn account_domain_manifest_permission_json_uses_dot_fqn() {
        let token = CanPublishSpaceDirectoryManifestForAccountDomain {
            dataspace: DataSpaceId::new(10),
            domain: DomainId::try_new("hbl", "sbp").expect("HBL domain"),
        };
        let payload = norito::json::to_json(&token).expect("serialize publisher permission");
        assert_eq!(payload, r#"{"dataspace":10,"domain":"hbl.sbp"}"#);
        assert_eq!(
            norito::json::from_str::<CanPublishSpaceDirectoryManifestForAccountDomain>(&payload)
                .expect("deserialize publisher permission"),
            token,
        );
    }
    #[test]
    fn sponsor_program_permissions_json_use_exact_program_id() {
        let sponsor = make_account_id();
        let token = CanEnrollFeeSponsorProgram {
            program_id: make_fee_sponsor_program_id(sponsor, "retail"),
        };
        let payload = norito::json::to_json(&token).expect("serialize enrollment permission");
        assert_eq!(
            payload,
            format!(r#"{{"program_id":"{}"}}"#, token.program_id),
        );
        assert_eq!(
            norito::json::from_str::<CanEnrollFeeSponsorProgram>(&payload)
                .expect("deserialize enrollment permission"),
            token,
        );
    }
}
