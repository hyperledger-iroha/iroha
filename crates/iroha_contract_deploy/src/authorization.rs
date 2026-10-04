//! Account existence and exact effective native deployment permissions.
use super::*;
use iroha::data_model::{
    alias_setup::{AccountAliasName, ResolvedAccountAliasV1},
    permission::Permission,
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias,
};
use std::collections::BTreeSet;

/// Concrete effective permission tokens observed before any deployment is submitted.
#[derive(Clone, Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct DeploymentAuthorization {
    /// The exact configured account was returned by the authenticated account read.
    pub account_exists: bool,
    /// Effective alias mutation token for this exact alias or its applicable parent scope.
    pub manage_alias_permission: Permission,
}

pub fn verify_account(client: &Client, authority: &AccountId) -> Result<()> {
    let account = client.client().get_account_read(authority)
        .wrap_err("deployment authority must already be registered and funded; acquire or use an owned alias namespace before deployment")?;
    if account.account_id != *authority {
        return Err(eyre!(
            "account read returned a different deployment authority"
        ));
    }
    Ok(())
}

pub fn read_authorization(
    client: &Client,
    authority: &AccountId,
    alias: &ContractAlias,
    dataspace_id: DataSpaceId,
) -> Result<DeploymentAuthorization> {
    match_permissions(
        &read_effective_permissions(client, authority)?,
        alias,
        dataspace_id,
    )
}

pub fn read_effective_permissions(
    client: &Client,
    authority: &AccountId,
) -> Result<BTreeSet<Permission>> {
    client.client().read_effective_permissions(authority)
}

#[cfg(test)]
#[path = "authorization_http_tests.rs"]
mod http_tests;

fn match_permissions(
    permissions: &BTreeSet<Permission>,
    alias: &ContractAlias,
    dataspace_id: DataSpaceId,
) -> Result<DeploymentAuthorization> {
    let name = AccountAliasName::try_new(
        alias.name_segment(),
        alias.domain_segment(),
        alias.dataspace_segment(),
    )?;
    let exact: Permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Alias(ResolvedAccountAliasV1::new(
            name.clone(),
            dataspace_id,
        )),
    }
    .into();
    let parent: Permission = CanManageAccountAlias {
        scope: name.domain_id().map_or(
            AccountAliasPermissionScope::Dataspace(dataspace_id),
            AccountAliasPermissionScope::Domain,
        ),
    }
    .into();
    let manage = if permissions.contains(&exact) {
        exact
    } else if permissions.contains(&parent) {
        parent
    } else {
        return Err(eyre!(
            "deployment authority lacks CanManageAccountAlias for `{alias}`; obtain the exact alias grant or its applicable {} grant",
            if alias.domain_segment().is_some() {
                "domain"
            } else {
                "dataspace"
            }
        ));
    };
    Ok(DeploymentAuthorization {
        account_exists: true,
        manage_alias_permission: manage,
    })
}

pub fn validate_authorization(
    evidence: &DeploymentAuthorization,
    alias: &ContractAlias,
    dataspace_id: DataSpaceId,
) -> Result<()> {
    if !evidence.account_exists {
        return Err(eyre!(
            "retained deployment lacks account-existence evidence"
        ));
    }
    match_permissions(
        &BTreeSet::from([evidence.manage_alias_permission.clone()]),
        alias,
        dataspace_id,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_creation_needs_only_exact_owned_alias_scope() -> Result<()> {
        let alias: ContractAlias = "coffee::merchant.universal".parse()?;
        let dataspace: Permission = CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Dataspace(DataSpaceId::UNIVERSAL),
        }
        .into();
        let domain: Permission = CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(
                iroha_model_base::domain::DomainId::try_new("merchant", "universal")?,
            ),
        }
        .into();
        assert!(
            match_permissions(&BTreeSet::from([dataspace]), &alias, DataSpaceId::UNIVERSAL)
                .is_err()
        );
        assert!(match_permissions(&BTreeSet::new(), &alias, DataSpaceId::UNIVERSAL).is_err());
        let evidence =
            match_permissions(&BTreeSet::from([domain]), &alias, DataSpaceId::UNIVERSAL)?;
        validate_authorization(&evidence, &alias, DataSpaceId::UNIVERSAL)?;
        assert!(
            validate_authorization(
                &evidence,
                &"coffee::another.universal".parse()?,
                DataSpaceId::UNIVERSAL
            )
            .is_err()
        );
        Ok(())
    }
    #[test]
    fn exact_alias_does_not_authorize_another_alias_or_dataspace() -> Result<()> {
        let alias: ContractAlias = "coffee::universal".parse()?;
        let exact: Permission = CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Alias(ResolvedAccountAliasV1::new(
                AccountAliasName::try_new("coffee", None::<&str>, "universal")?,
                DataSpaceId::UNIVERSAL,
            )),
        }
        .into();
        let permissions = BTreeSet::from([exact]);
        match_permissions(&permissions, &alias, DataSpaceId::UNIVERSAL)?;
        assert!(
            match_permissions(
                &permissions,
                &"tea::universal".parse()?,
                DataSpaceId::UNIVERSAL
            )
            .is_err()
        );
        assert!(match_permissions(&permissions, &alias, DataSpaceId::new(1)).is_err());
        Ok(())
    }
}
