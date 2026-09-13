//! Account existence and exact effective native deployment permissions.
use super::*;
use iroha::data_model::{
    alias_setup::{AccountAliasName, ResolvedAccountAliasV1},
    permission::Permission,
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias},
    smart_contract::CanRegisterSmartContractCode,
};
use std::collections::BTreeSet;

/// Concrete effective permission tokens observed before any deployment is submitted.
#[derive(Clone, Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct DeploymentAuthorization {
    /// The exact configured account was returned by the authenticated account read.
    pub account_exists: bool,
    /// Effective native registrar token, including its canonical unit payload.
    pub register_code_permission: Permission,
    /// Effective alias mutation token for this exact alias or its applicable parent scope.
    pub manage_alias_permission: Permission,
}

pub(super) fn verify_account(client: &Client, authority: &AccountId) -> Result<()> {
    let account = client.client().get_account_read(authority)
        .wrap_err("deployment authority must already be registered; onboarding and fee funding do not grant deployment permissions")?;
    if account.account_id != *authority {
        return Err(eyre!(
            "account read returned a different deployment authority"
        ));
    }
    Ok(())
}

pub(super) fn read_authorization(
    client: &Client,
    authority: &AccountId,
    alias: &ContractAlias,
    dataspace_id: DataSpaceId,
) -> Result<DeploymentAuthorization> {
    #[derive(norito::derive::JsonDeserialize)]
    struct Page {
        items: Vec<Permission>,
        total: u64,
    }
    let mut permissions = BTreeSet::new();
    for page_index in 0..32_u64 {
        let response = client.client().get_account_permissions_page_response(
            authority,
            500,
            page_index * 500,
        )?;
        if response.status().as_u16() != 200 {
            return Err(eyre!(
                "effective deployment permission read failed with HTTP {}",
                response.status()
            ));
        }
        let header = |name: &str| -> Result<&str> {
            let mut values = response.headers().get_all(name).iter();
            let value = values
                .next()
                .ok_or_else(|| eyre!("effective permission response has no {name}"))?;
            if values.next().is_some() {
                return Err(eyre!("effective permission response has duplicate {name}"));
            }
            value.to_str().map_err(Into::into)
        };
        if !header("content-type")?
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case("application/json")
            || header("x-iroha-account-permission-semantics")? != "effective-v1"
        {
            return Err(eyre!(
                "deployment permission response must advertise JSON effective-v1 semantics"
            ));
        }
        let count = |name: &str| -> Result<u64> { canonical_decimal_u64(header(name)?, name) };
        let attempted = count("x-iroha-fanout-routes-attempted")?;
        if attempted == 0
            || count("x-iroha-fanout-routes-succeeded")? != attempted
            || count("x-iroha-fanout-routes-failed")? != 0
            || count("x-iroha-fanout-routes-denied")? != 0
            || count("x-iroha-fanout-routes-unavailable")? != 0
            || count("x-iroha-fanout-routes-not-found")? != 0
        {
            return Err(eyre!(
                "effective deployment permission fanout is incomplete; repair account routing before retrying"
            ));
        }
        if response.body().len() > 4 * 1024 * 1024 {
            return Err(eyre!(
                "effective permission page exceeds fixed response bound"
            ));
        }
        let page: Page = norito::json::from_slice(response.body())?;
        // Public account-permission reads use the List fanout merger, whose total counts the
        // returned deduplicated page. It differs from the internal route's all-matched total.
        if page.total != page.items.len() as u64 {
            return Err(eyre!(
                "effective permission page total disagrees with its items"
            ));
        }
        if page.items.is_empty() {
            return match_permissions(&permissions, alias, dataspace_id);
        }
        permissions.extend(page.items);
        if permissions.len() > 16_000 {
            return Err(eyre!(
                "effective deployment permissions exceed fixed collection bound"
            ));
        }
    }
    Err(eyre!(
        "effective deployment permissions did not reach an empty complete page within the fixed traversal bound"
    ))
}

#[cfg(test)]
#[path = "authorization_http_tests.rs"]
mod http_tests;

fn match_permissions(
    permissions: &BTreeSet<Permission>,
    alias: &ContractAlias,
    dataspace_id: DataSpaceId,
) -> Result<DeploymentAuthorization> {
    let register: Permission = CanRegisterSmartContractCode.into();
    if !permissions.contains(&register) {
        return Err(eyre!(
            "deployment authority lacks CanRegisterSmartContractCode; fee funding or account onboarding cannot replace an authorized registrar grant"
        ));
    }
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
        register_code_permission: register,
        manage_alias_permission: manage,
    })
}

pub(super) fn validate_authorization(
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
        &BTreeSet::from([
            evidence.register_code_permission.clone(),
            evidence.manage_alias_permission.clone(),
        ]),
        alias,
        dataspace_id,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registrar_and_alias_permissions_are_exact_and_independent() -> Result<()> {
        let alias: ContractAlias = "coffee::merchant.universal".parse()?;
        let register: Permission = CanRegisterSmartContractCode.into();
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
            match_permissions(
                &BTreeSet::from([register.clone(), dataspace]),
                &alias,
                DataSpaceId::UNIVERSAL
            )
            .is_err()
        );
        assert!(
            match_permissions(
                &BTreeSet::from([domain.clone()]),
                &alias,
                DataSpaceId::UNIVERSAL
            )
            .is_err()
        );
        let evidence = match_permissions(
            &BTreeSet::from([register, domain]),
            &alias,
            DataSpaceId::UNIVERSAL,
        )?;
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
        let permissions = BTreeSet::from([CanRegisterSmartContractCode.into(), exact]);
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
