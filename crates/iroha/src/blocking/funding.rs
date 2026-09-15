//! Signed global balances and cumulative native transaction funding admission.
use super::Client;
use eyre::{Result, WrapErr as _, eyre};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, address::ChainDiscriminantGuard},
    asset::{AssetDefinitionId, AssetId},
    nexus::{FeeDebitSource, FeeSponsorProgramId},
    query::{
        account::prelude::FindAccountById,
        asset::prelude::{FindAssetById, FindAssetDefinitionById},
    },
};
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::FeeQuoteResponse;
use norito::json::{self, JsonSerialize, Value};
use std::collections::BTreeMap;
/// Signed account balance query result for one exact asset definition.
#[derive(Clone, Debug, JsonSerialize)]
pub struct BalanceReport {
    /// Exact network security identity.
    pub network_id: NetworkId,
    /// Public address codec profile.
    pub chain_discriminant: u16,
    /// Queried account; its existence was checked independently.
    pub account_id: AccountId,
    /// Exact asset definition; its existence was checked independently.
    pub asset_definition: AssetDefinitionId,
    /// Available quantity; an authenticated missing holding is zero.
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
    /// Exact global asset definition.
    pub asset_definition: AssetDefinitionId,
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
    pub fn balance(&self, asset_definition: &AssetDefinitionId) -> Result<BalanceReport> {
        let _profile = ChainDiscriminantGuard::enter(self.client().account_chain_discriminant);
        let client = self.client();
        let account = client.query_single(FindAccountById::new(self.client().account.clone())).wrap_err("balance requires an existing registered account and authenticated query access; fund or onboard this wallet first if it is new")?;
        let definition =
            client.query_single(FindAssetDefinitionById::new(asset_definition.clone())).wrap_err_with(|| format!("balance requires the exact asset definition {asset_definition} to exist and be readable"))?;
        if account.id != self.client().account || &definition.id != asset_definition {
            eyre::bail!(
                "balance prerequisite query returned a substituted account or asset definition"
            );
        }
        if definition.balance_scope_policy != iroha_data_model::asset::AssetBalancePolicy::Global {
            eyre::bail!("this balance operation requires an asset with global balance scope");
        }
        let id = AssetId::new(asset_definition.clone(), self.client().account.clone());
        let amount = match client.query_single(FindAssetById::new(id.clone())) {
            Ok(asset) if asset.id == id => asset.value,
            Ok(_) => eyre::bail!("balance query returned a substituted asset identity"),
            Err(crate::query::QueryError::Validation(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::NotFound,
                ),
            )) => Quantity::from(0_u32),
            Err(error) => return Err(eyre!(error).wrap_err("cannot establish the exact asset holding; unavailable or malformed reads are never a zero balance")),
        };
        Ok(BalanceReport {
            network_id: self.client().network_id,
            chain_discriminant: (self.client().account_chain_discriminant),
            account_id: self.client().account.clone(),
            asset_definition: asset_definition.clone(),
            amount,
        })
    }
    /// Check cumulative principal and maximum fees before a sequence of transactions.
    ///
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
        principal: &BTreeMap<AssetDefinitionId, Quantity>,
        quotes: &[FeeQuoteResponse],
    ) -> Result<Vec<FundingRequirement>> {
        let (required, mut sponsors) =
            aggregate_funding(&self.client().account, principal, quotes)?;
        let mut result = Vec::with_capacity(required.len() + sponsors.len());
        for (asset_definition, required) in required {
            let available = self.balance(&asset_definition)?.amount;
            ensure_funded(
                &asset_definition,
                &required,
                &available,
                "account principal plus authority fee maxima",
            )?;
            result.push(FundingRequirement {
                debit_source: FeeDebitSource::Account(self.client().account.clone()),
                asset_definition,
                required,
                available,
            });
        }
        result.append(&mut sponsors);
        Ok(result)
    }
}
fn add_quantity(
    required: &mut BTreeMap<AssetDefinitionId, Quantity>,
    asset: &AssetDefinitionId,
    amount: &Quantity,
) -> Result<()> {
    let total = required.entry(asset.clone()).or_insert_with(Quantity::zero);
    *total = total.checked_add(amount)?;
    Ok(())
}

type AccountRequirements = BTreeMap<AssetDefinitionId, Quantity>;
fn aggregate_funding(
    authority: &AccountId,
    principal: &AccountRequirements,
    quotes: &[FeeQuoteResponse],
) -> Result<(AccountRequirements, Vec<FundingRequirement>)> {
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
                add_quantity(
                    &mut required,
                    &component.asset_definition_id,
                    &component.max_amount,
                )?;
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
                required,
                available,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((required, sponsors))
}
fn ensure_funded(
    asset: &AssetDefinitionId,
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
