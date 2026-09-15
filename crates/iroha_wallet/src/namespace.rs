//! Exact paid domain-namespace requests built from public native policy and authoritative names.

use eyre::{Result, WrapErr as _, eyre};
use iroha::{blocking, config::Config, sns::SnsNamespacePath};
use iroha_data_model::{
    account::{AccountId, address::ChainDiscriminantGuard},
    alias_setup::{
        AliasDomainIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1, AliasQuoteGuardV1,
        AliasSetupPlanRequestV1, ResolvedDomainV1,
    },
    asset::AssetDefinitionId,
    isi::alias_setup::EnsureAlias,
    sns::{
        DATASPACE_ALIAS_SUFFIX_ID, DOMAIN_NAME_SUFFIX_ID, NameRecordV1, NameSelectorV1, NameStatus,
        SuffixPolicyV1, pricing::quote_lease_price,
    },
};
use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::status::NexusDataspaceCatalogStatus;
use std::time::{Duration, Instant};

const MAX_QUOTE_LIFETIME_MS: u64 = 300_000;

/// One year of namespace rent, with the exact native request ready for fee quoting and planning.
#[derive(Debug, Clone)]
pub struct DomainNamespaceQuote {
    /// Exact owner-bound, one-year native namespace request.
    pub request: AliasSetupPlanRequestV1,
    /// Canonical fully qualified domain.
    pub domain: String,
    /// Exact advertised one-year rent, separate from transaction fees.
    pub rent: Quantity,
    /// Asset definition in which namespace rent is paid.
    pub payment_asset: AssetDefinitionId,
    /// Node-time deadline carried by the signed rent guard.
    pub valid_until_ms: u64,
}

/// Read current policy and resolve a full domain into one bounded, owner-bound native request.
///
/// The exact network/profile is checked at the configured HTTPS origin (or explicit loopback).
/// Quote time comes from a fresh native status snapshot. The native setup planner independently
/// revalidates the retained name/id pair, ownership, active policy and exact rent before signing.
/// This function never creates a transaction, submits, or writes a journal.
///
/// # Errors
/// Rejects malformed domains, changed network/profile, inactive or inconsistent names/policy,
/// unavailable one-year terms, stale reads, invalid payment currency and unrepresentable deadlines.
pub fn prepare_domain_request(config: &Config, domain: &str) -> Result<DomainNamespaceQuote> {
    let domain =
        DomainId::parse_fully_qualified(domain).wrap_err("namespace must be domain.dataspace")?;
    let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
    let bootstrap = blocking::account_bootstrap::Client::new(
        config.torii_api_url.clone(),
        config.torii_request_timeout,
    )?;
    let capabilities = bootstrap.capabilities()?;
    if capabilities.network_id != config.network_id
        || capabilities.network_prefix != config.account_chain_discriminant
    {
        return Err(eyre!(
            "namespace endpoint changed the wallet's exact network or account profile"
        ));
    }
    let client = blocking::Client::new(config.clone())?;
    let status = client
        .status()
        .get()
        .wrap_err("read current namespace network time and catalog")?;
    let started = Instant::now();
    let name = domain.dataspace().as_ref();
    let catalog_id = catalog_dataspace(&status.dataspace_catalog, name)?;
    let dataspace_id = if let Some(id) = catalog_id {
        id
    } else if name == "universal" {
        DataSpaceId::UNIVERSAL
    } else {
        let record = client
            .client()
            .sns()
            .get_name(SnsNamespacePath::Dataspace, name)
            .wrap_err("resolve active namespace dataspace")?;
        active_dataspace(&record, name, status.observed_at_ms)?
    };
    if name == "universal" && dataspace_id != DataSpaceId::UNIVERSAL {
        return Err(eyre!(
            "public catalog remapped the reserved universal dataspace"
        ));
    }
    let policy = client
        .client()
        .sns()
        .get_policy(DOMAIN_NAME_SUFFIX_ID)
        .wrap_err("read current domain namespace pricing policy")?;
    let quote = quote_domain(
        &policy,
        domain,
        dataspace_id,
        config.account.clone(),
        status.observed_at_ms,
        config.transaction_ttl,
    )?;
    let lifetime = quote.valid_until_ms - status.observed_at_ms;
    if started.elapsed().as_millis() >= u128::from(lifetime / 2) {
        return Err(eyre!(
            "namespace reads consumed the rent quote lifetime; retry with a fresh policy"
        ));
    }
    Ok(quote)
}

fn catalog_dataspace(
    catalog: &[NexusDataspaceCatalogStatus],
    name: &str,
) -> Result<Option<DataSpaceId>> {
    let mut found = None;
    for entry in catalog.iter().filter(|entry| entry.alias == name) {
        let id = DataSpaceId::new(entry.dataspace_id);
        if found.is_some_and(|previous| previous != id) {
            return Err(eyre!(
                "public catalog contains conflicting ids for dataspace `{name}`"
            ));
        }
        found = Some(id);
    }
    if let Some(id) = found
        && catalog
            .iter()
            .any(|entry| entry.dataspace_id == id.as_u64() && entry.alias != name)
    {
        return Err(eyre!(
            "public catalog maps dataspace id {id} to more than one name"
        ));
    }
    Ok(found)
}

fn active_dataspace(record: &NameRecordV1, name: &str, now_ms: u64) -> Result<DataSpaceId> {
    let expected = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, name)?;
    if record.selector != expected
        || record.name_hash != expected.name_hash()
        || !matches!(record.status, NameStatus::Active)
        || record.ownership_generation == 0
        || record.registered_at_ms > now_ms
        || record.expires_at_ms <= now_ms
    {
        return Err(eyre!(
            "dataspace lookup is not the exact active SNS name `{name}`"
        ));
    }
    // This canonical SNS metadata field pins preconfigured numeric mappings. Names without it
    // use the protocol selector-derived id; the planner checks both live catalog directions.
    if let Some(value) = record.metadata.get("sns.dataspace_id") {
        return Ok(DataSpaceId::new(
            norito::json::from_str::<u64>(value.get())
                .wrap_err("dataspace record has invalid numeric mapping")?,
        ));
    }
    Ok(DataSpaceId::from_hash(&expected.name_hash()))
}

fn quote_domain(
    policy: &SuffixPolicyV1,
    domain: DomainId,
    dataspace_id: DataSpaceId,
    owner: AccountId,
    observed_at_ms: u64,
    ttl: Duration,
) -> Result<DomainNamespaceQuote> {
    if observed_at_ms == 0 {
        return Err(eyre!(
            "namespace status did not provide a current network timestamp"
        ));
    }
    let lifetime_ms = u64::try_from(ttl.as_millis())
        .unwrap_or(u64::MAX)
        .min(MAX_QUOTE_LIFETIME_MS);
    if lifetime_ms < 1_000 {
        return Err(eyre!(
            "namespace transaction lifetime must be at least one second"
        ));
    }
    let valid_until_ms = observed_at_ms
        .checked_add(lifetime_ms)
        .ok_or_else(|| eyre!("namespace quote deadline overflow"))?;
    let selector = NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, domain.to_string())?;
    let price = quote_lease_price(policy, &selector, 1, None)?;
    let guard = AliasQuoteGuardV1 {
        expected_policy_version: policy.policy_version,
        expected_payment_asset: price.payment_asset.clone(),
        max_amount: price.amount.clone(),
        valid_until_ms,
    };
    Ok(DomainNamespaceQuote {
        domain: domain.to_string(),
        rent: price.amount,
        payment_asset: price.payment_asset,
        valid_until_ms,
        request: AliasSetupPlanRequestV1::new(vec![EnsureAlias::new(
            AliasIntentV1::Domain(AliasDomainIntentV1 {
                domain: ResolvedDomainV1::new(domain, dataspace_id),
                owner,
            }),
            AliasLeaseAcquisitionV1::new(1, Some(price.pricing_class)),
            guard,
        )]),
    })
}

#[cfg(test)]
mod tests;
