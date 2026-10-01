//! Signed exact-scope balances and cumulative native transaction funding admission.
use super::Client;
use eyre::{Result, WrapErr as _, eyre};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, address::ChainDiscriminantGuard},
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinitionId, AssetId},
    nexus::{FeeDebitSource, FeeSponsorProgramId},
    query::{
        account::prelude::FindAccountById,
        asset::prelude::{FindAssetById, FindAssetDefinitionById},
        error::{FindError, QueryExecutionFail},
    },
};
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::FeeQuoteResponse;
use norito::json::{self, JsonSerialize, Value};
use std::collections::BTreeMap;
/// Signed account balance query result for one exact scoped asset holding.
#[derive(Clone, Debug, JsonSerialize)]
pub struct BalanceReport {
    /// Exact network security identity.
    pub network_id: NetworkId,
    /// Public address codec profile.
    pub chain_discriminant: u16,
    /// Exact account, asset definition, and balance scope queried.
    pub asset_id: AssetId,
    /// Available quantity; typed absence of this exact holding is zero.
    pub amount: Quantity,
}
impl BalanceReport {
    /// Encode public evidence under its retained address profile.
    ///
    /// # Errors
    /// Returns a native JSON encoding error.
    pub fn to_json(&self) -> Result<Value> {
        let _profile = ChainDiscriminantGuard::enter(self.chain_discriminant);
        Ok(json::to_value(self)?)
    }
}

/// Read-only solvency observation for one exact account or sponsor funding source and asset.
#[derive(Clone, Debug, JsonSerialize)]
pub struct FundingRequirement {
    /// Exact account or isolated sponsor vault to debit.
    pub debit_source: FeeDebitSource,
    /// Exact asset definition.
    pub asset_definition: AssetDefinitionId,
    /// Exact balance bucket; sponsor vaults support only global assets.
    pub balance_scope: AssetBalanceScope,
    /// Aggregate principal and maximum charges for this debit source.
    pub required: Quantity,
    /// Current account balance or conservative sponsor capacity.
    pub available: Quantity,
}

impl Client {
    /// Read one exact asset holding; absent accounts or definitions never become a zero balance.
    ///
    /// # Errors
    /// Returns authentication, routing or typed query failures.
    pub fn balance(&self, asset_id: &AssetId) -> Result<BalanceReport> {
        let _profile = ChainDiscriminantGuard::enter(self.client().account_chain_discriminant);
        self.require_balance_authority(asset_id)?;
        self.require_balance_account()?;
        let policy = self.balance_policy(asset_id.definition())?;
        validate_scope(asset_id, policy)?;
        let amount = self.exact_holding(asset_id)?;
        Ok(BalanceReport {
            network_id: self.client().network_id,
            chain_discriminant: self.client().account_chain_discriminant,
            asset_id: asset_id.clone(),
            amount,
        })
    }
    fn require_balance_authority(&self, asset_id: &AssetId) -> Result<()> {
        if asset_id.account() != &self.client().account {
            eyre::bail!("funding requires an exact holding of the configured authority");
        }
        Ok(())
    }
    fn require_balance_account(&self) -> Result<()> {
        let account = self.client().query_single(FindAccountById::new(self.client().account.clone())).wrap_err("balance requires an existing registered account and authenticated query access; fund or onboard this wallet first if it is new")?;
        if account.id != self.client().account {
            eyre::bail!("balance prerequisite query returned a substituted account");
        }
        Ok(())
    }
    fn balance_policy(&self, asset_definition: &AssetDefinitionId) -> Result<AssetBalancePolicy> {
        let definition = self.client().query_single(FindAssetDefinitionById::new(asset_definition.clone())).wrap_err_with(|| format!("balance requires the exact asset definition {asset_definition} to exist and be readable"))?;
        if &definition.id != asset_definition {
            eyre::bail!("balance prerequisite query returned a substituted asset definition");
        }
        Ok(definition.balance_scope_policy)
    }
    fn exact_holding(&self, id: &AssetId) -> Result<Quantity> {
        match self.client().query_single(FindAssetById::new(id.clone())) {
            Ok(asset) if &asset.id == id => Ok(asset.value),
            Ok(_) => eyre::bail!("balance query returned a substituted asset identity"),
            Err(crate::query::QueryError::Validation(
                iroha_data_model::ValidationFail::QueryFailed(QueryExecutionFail::Find(
                    FindError::Asset(missing),
                )),
            )) if missing.as_ref() == id => Ok(Quantity::zero()),
            Err(error) => Err(eyre!(error).wrap_err("cannot establish the exact asset holding; unavailable or malformed reads are never a zero balance")),
        }
    }
    /// Check cumulative principal and maximum fees before a sequence of transactions.
    ///
    /// Principal identities explicitly bind the authority and balance scope. Each fee uses the
    /// queried definition policy and its quote's exact route: global balances are shared across
    /// routes, while restricted balances remain separate for each dataspace.
    /// Authority balances are queried exactly. Sponsor requirements use the most conservative
    /// quoted vault and epoch capacities for each program revision. Block capacities remain
    /// per-transaction because sequential stages may finalize in different blocks. This read
    /// does not reserve funds; every transaction still requires its exact SDK quote and admission.
    ///
    /// # Errors
    /// Rejects malformed quotes, changed payers/revisions, incomplete balance reads, arithmetic
    /// overflow, or insufficient aggregate funding with required and available quantities.
    pub fn check_funding(
        &self,
        principal: &BTreeMap<AssetId, Quantity>,
        quotes: &[FeeQuoteResponse],
    ) -> Result<Vec<FundingRequirement>> {
        let _profile = ChainDiscriminantGuard::enter(self.client().account_chain_discriminant);
        // Reject substituted principal owners and malformed quotes before any network read.
        for asset in principal.keys() {
            self.require_balance_authority(asset)?;
        }
        for quote in quotes {
            quote
                .validate_for_authority(&self.client().account)
                .map_err(|error| eyre!(error))?;
        }
        let definitions = principal
            .keys()
            .map(|id| id.definition().clone())
            .chain(quotes.iter().flat_map(|quote| {
                quote
                    .components
                    .iter()
                    .map(|component| component.asset_definition_id.clone())
            }))
            .collect::<std::collections::BTreeSet<_>>();
        let mut policies = BTreeMap::new();
        if !definitions.is_empty() {
            self.require_balance_account()?;
        }
        for definition in definitions {
            policies.insert(definition.clone(), self.balance_policy(&definition)?);
        }
        let (required, mut sponsors) =
            aggregate_funding(&self.client().account, principal, quotes, &policies)?;
        let mut result = Vec::with_capacity(required.len() + sponsors.len());
        for (asset_id, required) in required {
            let available = self.exact_holding(&asset_id)?;
            ensure_funded(
                &asset_id,
                &required,
                &available,
                "account principal plus authority fee maxima",
            )?;
            result.push(FundingRequirement {
                debit_source: FeeDebitSource::Account(self.client().account.clone()),
                asset_definition: asset_id.definition().clone(),
                balance_scope: *asset_id.scope(),
                required,
                available,
            });
        }
        result.append(&mut sponsors);
        Ok(result)
    }
}
fn add_quantity<K: Ord + Clone>(
    required: &mut BTreeMap<K, Quantity>,
    asset: &K,
    amount: &Quantity,
) -> Result<()> {
    let total = required.entry(asset.clone()).or_insert_with(Quantity::zero);
    *total = total.checked_add(amount)?;
    Ok(())
}

fn validate_scope(asset: &AssetId, policy: AssetBalancePolicy) -> Result<()> {
    if !matches!(
        (policy, asset.scope()),
        (AssetBalancePolicy::Global, AssetBalanceScope::Global)
            | (
                AssetBalancePolicy::DataspaceRestricted,
                AssetBalanceScope::Dataspace(_)
            )
    ) {
        eyre::bail!("asset holding scope differs from the registered balance policy: {asset}");
    }
    Ok(())
}
fn required_policy(
    policies: &BTreeMap<AssetDefinitionId, AssetBalancePolicy>,
    definition: &AssetDefinitionId,
) -> Result<AssetBalancePolicy> {
    policies
        .get(definition)
        .copied()
        .ok_or_else(|| eyre!("funding lacks the exact registered asset policy for {definition}"))
}
type AccountRequirements = BTreeMap<AssetId, Quantity>;
fn aggregate_funding(
    authority: &AccountId,
    principal: &AccountRequirements,
    quotes: &[FeeQuoteResponse],
    policies: &BTreeMap<AssetDefinitionId, AssetBalancePolicy>,
) -> Result<(AccountRequirements, Vec<FundingRequirement>)> {
    for asset in principal.keys() {
        if asset.account() != authority {
            eyre::bail!("funding principal belongs to a different authority");
        }
        validate_scope(asset, required_policy(policies, asset.definition())?)?;
    }
    let mut required = principal.clone();
    let mut sponsored: BTreeMap<
        (FeeSponsorProgramId, AssetDefinitionId),
        (u64, Quantity, Quantity),
    > = BTreeMap::new();
    for quote in quotes {
        quote
            .validate_for_authority(authority)
            .map_err(|error| eyre!(error))?;
        if let Some((program, revision)) = quote.intent.sponsor_program() {
            let mut stage = BTreeMap::new();
            for component in &quote.components {
                if required_policy(policies, &component.asset_definition_id)?
                    != AssetBalancePolicy::Global
                {
                    eyre::bail!("sponsor funding requires a registered global asset policy");
                }
                add_quantity(
                    &mut stage,
                    &component.asset_definition_id,
                    &component.max_amount,
                )?;
            }
            for capacity in &quote.capacities {
                let available = capacity
                    .vault_balance
                    .try_sub(&capacity.reserve_floor)?
                    .min(capacity.program_epoch_remaining.clone())
                    .min(capacity.beneficiary_epoch_remaining.clone());
                let entry = sponsored
                    .entry((program.clone(), capacity.asset_definition_id.clone()))
                    .or_insert_with(|| (revision, Quantity::zero(), available.clone()));
                if entry.0 != revision {
                    eyre::bail!("native funding quotes disagree on the immutable sponsor revision");
                }
                entry.1 = entry.1.checked_add(&stage[&capacity.asset_definition_id])?;
                entry.2 = entry.2.clone().min(available);
            }
        } else {
            for component in &quote.components {
                let scope = match required_policy(policies, &component.asset_definition_id)? {
                    AssetBalancePolicy::Global => AssetBalanceScope::Global,
                    AssetBalancePolicy::DataspaceRestricted => {
                        AssetBalanceScope::Dataspace(quote.observation.route_dataspace_id)
                    }
                };
                let asset = AssetId::with_scope(
                    component.asset_definition_id.clone(),
                    authority.clone(),
                    scope,
                );
                add_quantity(&mut required, &asset, &component.max_amount)?;
            }
        }
    }
    let sponsors = sponsored
        .into_iter()
        .map(|((program, asset_definition), (_, required, available))| {
            ensure_funded(
                &asset_definition,
                &required,
                &available,
                "sponsor vault reserve and epoch capacity",
            )?;
            Ok(FundingRequirement {
                debit_source: FeeDebitSource::SponsorProgram(program),
                asset_definition,
                balance_scope: AssetBalanceScope::Global,
                required,
                available,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((required, sponsors))
}
fn ensure_funded(
    asset: &impl std::fmt::Display,
    required: &Quantity,
    available: &Quantity,
    kind: &str,
) -> Result<()> {
    if available < required {
        eyre::bail!(
            "insufficient funding for {asset}: required {required} ({kind}), available {available}"
        );
    }
    Ok(())
}
#[cfg(test)]
#[path = "funding_tests.rs"]
mod tests;
