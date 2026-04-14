//! Module with permission related functionality.

use std::{borrow::ToOwned as _, collections::BTreeSet, vec::Vec};

use iroha_executor_data_model::permission::Permission;

use crate::{
    Execute,
    prelude::Context,
    smart_contract::{
        data_model::{executor::Result, permission::Permission as PermissionObject, prelude::*},
        prelude::*,
    },
};

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
                match self { $(
                    AnyPermission::$token_ty(permission) => permission.validate_grant(authority, context, host), )*
                }
            }

            fn validate_revoke(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
                match self { $(
                    AnyPermission::$token_ty(permission) => permission.validate_revoke(authority, context, host), )*
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
    iroha_executor_data_model::permission::account::{CanResolveAccountAlias},

    iroha_executor_data_model::permission::asset_definition::{CanUnregisterAssetDefinition},
    iroha_executor_data_model::permission::asset_definition::{CanModifyAssetDefinitionMetadata},

    iroha_executor_data_model::permission::asset::{CanMintAssetWithDefinition},
    iroha_executor_data_model::permission::asset::{CanBurnAssetWithDefinition},
    iroha_executor_data_model::permission::asset::{CanTransferAssetWithDefinition},
    iroha_executor_data_model::permission::asset::{CanModifyAssetMetadataWithDefinition},
    iroha_executor_data_model::permission::asset::{CanMintAsset},
    iroha_executor_data_model::permission::asset::{CanBurnAsset},
    iroha_executor_data_model::permission::asset::{CanTransferAsset},
    iroha_executor_data_model::permission::asset::{CanModifyAssetMetadata},

    iroha_executor_data_model::permission::nft::{CanRegisterNft},
    iroha_executor_data_model::permission::nft::{CanUnregisterNft},
    iroha_executor_data_model::permission::nft::{CanTransferNft},
    iroha_executor_data_model::permission::nft::{CanModifyNftMetadata},

    iroha_executor_data_model::permission::parameter::{CanSetParameters},
    iroha_executor_data_model::permission::role::{CanManageRoles},

    iroha_executor_data_model::permission::trigger::{CanRegisterTrigger},
    iroha_executor_data_model::permission::trigger::{CanUnregisterTrigger},
    iroha_executor_data_model::permission::trigger::{CanModifyTrigger},
    iroha_executor_data_model::permission::trigger::{CanExecuteTrigger},
    iroha_executor_data_model::permission::trigger::{CanModifyTriggerMetadata},

    iroha_executor_data_model::permission::executor::{CanUpgradeExecutor},

    iroha_executor_data_model::permission::smart_contract::{CanRegisterSmartContractCode},
    iroha_executor_data_model::permission::sorafs::{CanRegisterSorafsPin},
    iroha_executor_data_model::permission::sorafs::{CanApproveSorafsPin},
    iroha_executor_data_model::permission::sorafs::{CanRetireSorafsPin},
    iroha_executor_data_model::permission::sorafs::{CanBindSorafsAlias},
    iroha_executor_data_model::permission::sorafs::{CanDeclareSorafsCapacity},
    iroha_executor_data_model::permission::sorafs::{CanSubmitSorafsTelemetry},
    iroha_executor_data_model::permission::sorafs::{CanFileSorafsCapacityDispute},
    iroha_executor_data_model::permission::sorafs::{CanIssueSorafsReplicationOrder},
    iroha_executor_data_model::permission::sorafs::{CanCompleteSorafsReplicationOrder},
    iroha_executor_data_model::permission::sorafs::{CanSetSorafsPricing},
    iroha_executor_data_model::permission::sorafs::{CanUpsertSorafsProviderCredit},
    iroha_executor_data_model::permission::sorafs::{CanRegisterSorafsProviderOwner},
    iroha_executor_data_model::permission::sorafs::{CanUnregisterSorafsProviderOwner},
    iroha_executor_data_model::permission::soranet::{CanIngestSoranetPrivacy},
    iroha_executor_data_model::permission::nexus::{CanPublishSpaceDirectoryManifest},
    iroha_executor_data_model::permission::nexus::{CanUseFeeSponsor},
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

/// Trait that should be implemented for all permission tokens.
/// Provides a function to check validity of [`Grant`] and [`Revoke`]
/// instructions containing implementing permission.
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
    use iroha_executor_data_model::permission::executor::CanUpgradeExecutor;

    use super::*;

    impl ValidateGrantRevoke for CanUpgradeExecutor {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
    }
}

mod smart_contract {
    use iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode;

    use super::*;

    impl ValidateGrantRevoke for CanRegisterSmartContractCode {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
    }
}

mod nexus {
    use iroha_executor_data_model::permission::nexus::{
        CanPublishSpaceDirectoryManifest, CanUseFeeSponsor,
    };

    use super::*;

    impl ValidateGrantRevoke for CanPublishSpaceDirectoryManifest {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }

        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
    }

    #[derive(Debug, Clone)]
    struct SponsorAccount<'a> {
        sponsor: &'a AccountId,
    }

    impl PassCondition for SponsorAccount<'_> {
        fn validate(&self, authority: &AccountId, _host: &Iroha, _context: &Context) -> Result {
            if authority == self.sponsor {
                return Ok(());
            }
            Err(ValidationFail::NotPermitted(
                "only the sponsor account may grant or revoke fee sponsorship".to_owned(),
            ))
        }
    }

    impl<'a> From<&'a CanUseFeeSponsor> for SponsorAccount<'a> {
        fn from(value: &'a CanUseFeeSponsor) -> Self {
            Self {
                sponsor: &value.sponsor,
            }
        }
    }

    impl ValidateGrantRevoke for CanUseFeeSponsor {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            SponsorAccount::from(self).validate(authority, host, context)
        }

        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            SponsorAccount::from(self).validate(authority, host, context)
        }
    }
}

mod sorafs {
    use iroha_executor_data_model::permission::sorafs::{
        CanApproveSorafsPin, CanBindSorafsAlias, CanCompleteSorafsReplicationOrder,
        CanDeclareSorafsCapacity, CanFileSorafsCapacityDispute, CanIssueSorafsReplicationOrder,
        CanRegisterSorafsPin, CanRegisterSorafsProviderOwner, CanRetireSorafsPin,
        CanSetSorafsPricing, CanSubmitSorafsTelemetry, CanUnregisterSorafsProviderOwner,
        CanUpsertSorafsProviderCredit,
    };

    use super::*;

    impl_owned_permission!(
        CanRegisterSorafsPin,
        CanApproveSorafsPin,
        CanRetireSorafsPin,
        CanBindSorafsAlias,
        CanDeclareSorafsCapacity,
        CanSubmitSorafsTelemetry,
        CanFileSorafsCapacityDispute,
        CanIssueSorafsReplicationOrder,
        CanCompleteSorafsReplicationOrder,
        CanSetSorafsPricing,
        CanUpsertSorafsProviderCredit,
        CanRegisterSorafsProviderOwner,
        CanUnregisterSorafsProviderOwner,
    );
}

mod soranet {
    use iroha_executor_data_model::permission::soranet::CanIngestSoranetPrivacy;

    use super::*;

    impl_owned_permission!(CanIngestSoranetPrivacy);
}

mod peer {
    use iroha_executor_data_model::permission::peer::{
        CanManageLaneRelayEmergency, CanManagePeers,
    };

    use super::*;

    impl ValidateGrantRevoke for CanManagePeers {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanManageLaneRelayEmergency {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
    }
}

mod role {
    use iroha_executor_data_model::permission::role::CanManageRoles;

    use super::*;

    impl ValidateGrantRevoke for CanManageRoles {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
    }
}

mod parameter {
    //! Module with pass conditions for parameter related tokens
    use iroha_executor_data_model::permission::parameter::CanSetParameters;

    use super::*;

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

pub mod asset {
    //! Module with pass conditions for asset related tokens

    use iroha_executor_data_model::permission::asset::{
        CanBurnAsset, CanBurnAssetWithDefinition, CanMintAsset, CanMintAssetWithDefinition,
        CanModifyAssetMetadata, CanModifyAssetMetadataWithDefinition, CanTransferAsset,
        CanTransferAssetWithDefinition,
    };

    use super::*;

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

    impl ValidateGrantRevoke for CanMintAssetWithDefinition {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            super::asset_definition::Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            super::asset_definition::Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanBurnAssetWithDefinition {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            super::asset_definition::Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            super::asset_definition::Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanTransferAssetWithDefinition {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            super::asset_definition::Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            super::asset_definition::Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanModifyAssetMetadataWithDefinition {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            super::asset_definition::Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            super::asset_definition::Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanMintAsset {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanBurnAsset {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanTransferAsset {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    macro_rules! impl_froms {
        ($($name:ty),+ $(,)?) => {$(
            impl<'t> From<&'t $name> for Owner<'t> {
                fn from(value: &'t $name) -> Self {
                    Self { asset: &value.asset}
                }
            })+
        };
    }

    impl_froms!(
        CanMintAsset,
        CanBurnAsset,
        CanTransferAsset,
        CanModifyAssetMetadata
    );

    impl ValidateGrantRevoke for CanModifyAssetMetadata {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }
}

pub mod asset_definition {
    //! Module with pass conditions for asset definition related tokens

    use iroha_executor_data_model::permission::asset_definition::{
        CanModifyAssetDefinitionMetadata, CanUnregisterAssetDefinition,
    };

    use super::*;
    use crate::smart_contract::{
        Iroha,
        data_model::{isi::error::InstructionExecutionError, query::error::FindError},
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

    impl ValidateGrantRevoke for CanUnregisterAssetDefinition {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanModifyAssetDefinitionMetadata {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
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
        iroha_executor_data_model::permission::asset::CanMintAssetWithDefinition,
        iroha_executor_data_model::permission::asset::CanBurnAssetWithDefinition,
        iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition,
        iroha_executor_data_model::permission::asset::CanModifyAssetMetadataWithDefinition,
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
    use iroha_executor_data_model::permission::nft::{
        CanModifyNftMetadata, CanRegisterNft, CanTransferNft, CanUnregisterNft,
    };

    use super::*;
    use crate::smart_contract::{
        Iroha,
        data_model::{isi::error::InstructionExecutionError, query::error::FindError},
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

    impl ValidateGrantRevoke for CanRegisterNft {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            super::domain::Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            super::domain::Owner::from(self).validate(authority, host, context)
        }
    }

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

    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias, CanModifyAccountMetadata,
        CanRegisterAccount, CanReplaceAccountController, CanResolveAccountAlias,
        CanUnregisterAccount,
    };

    use super::*;

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

    fn validate_dataspace_alias_owner(
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

    impl ValidateGrantRevoke for CanRegisterAccount {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            super::domain::Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            super::domain::Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanUnregisterAccount {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanModifyAccountMetadata {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanReplaceAccountController {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanResolveAccountAlias {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            match &self.scope {
                AccountAliasPermissionScope::Domain(domain) => {
                    validate_account_alias_domain_owner(domain, authority, context, host)
                }
                AccountAliasPermissionScope::Dataspace(dataspace) => {
                    validate_dataspace_alias_owner(*dataspace, authority, host)
                }
            }
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

    impl ValidateGrantRevoke for CanManageAccountAlias {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            match &self.scope {
                AccountAliasPermissionScope::Domain(domain) => {
                    validate_account_alias_domain_owner(domain, authority, context, host)
                }
                AccountAliasPermissionScope::Dataspace(dataspace) => {
                    validate_dataspace_alias_owner(*dataspace, authority, host)
                }
            }
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
    );
}

pub mod trigger {
    //! Module with pass conditions for trigger related tokens
    use iroha_executor_data_model::permission::trigger::{
        CanExecuteTrigger, CanModifyTrigger, CanModifyTriggerMetadata, CanRegisterTrigger,
        CanUnregisterTrigger,
    };

    use super::*;
    use crate::data_model::{
        isi::error::InstructionExecutionError,
        query::{error::FindError, trigger::FindTriggers},
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

    impl ValidateGrantRevoke for CanRegisterTrigger {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            super::account::Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            super::account::Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanExecuteTrigger {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanUnregisterTrigger {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanModifyTrigger {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanModifyTriggerMetadata {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

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
    use iroha_executor_data_model::permission::{
        domain::{CanModifyDomainMetadata, CanRegisterDomain, CanUnregisterDomain},
        nft::CanRegisterNft,
    };
    use iroha_smart_contract::data_model::{
        isi::error::InstructionExecutionError, query::error::FindError,
    };

    use super::*;

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

    impl ValidateGrantRevoke for CanRegisterDomain {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            OnlyGenesis::from(self).validate(authority, host, context)
        }
    }
    impl ValidateGrantRevoke for CanUnregisterDomain {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

    impl ValidateGrantRevoke for CanModifyDomainMetadata {
        fn validate_grant(&self, authority: &AccountId, context: &Context, host: &Iroha) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
        fn validate_revoke(
            &self,
            authority: &AccountId,
            context: &Context,
            host: &Iroha,
        ) -> Result {
            Owner::from(self).validate(authority, host, context)
        }
    }

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
    use std::{num::NonZeroU64, vec::Vec};

    use iroha_crypto::PublicKey;
    use iroha_executor_data_model::permission::{
        asset::CanMintAssetWithDefinition, domain::CanRegisterDomain, peer::CanManagePeers,
    };

    use super::{OnlyGenesis, PassCondition, has_permission_in_roles, permission_owned_in_sources};
    use crate::{
        data_model::ValidationFail,
        prelude::Context,
        smart_contract::{
            Iroha,
            data_model::{
                block::BlockHeader,
                permission::Permission as PermissionObject,
                prelude::{AccountId, AssetDefinitionId, RoleId},
            },
        },
    };

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
}
