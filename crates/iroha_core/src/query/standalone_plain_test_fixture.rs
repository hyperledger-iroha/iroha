//! Explicit public-ballot economics and real ISI funding for isolated tests.
//!
//! Context creation precedes referendum insertion. These helpers neither submit a ballot nor
//! create finality, verified execution, a confidential position or anonymous-voter authority.

use crate::{
    smartcontracts::Execute,
    state::{StateTransaction, WorldReadOnly},
};
use iroha_data_model::{
    account::{Account, AccountId},
    asset::{AssetBalancePolicy, AssetDefinition, AssetId},
    governance::conviction::{PlainConvictionPolicyV1, PlainVotingContextV1},
    isi::{Mint, Register},
};
use iroha_primitives::numeric::{NumericSpec, Quantity};
use mv::storage::StorageReadOnly;

/// Freeze explicitly selected test economics before creating a PLAIN referendum.
#[must_use]
pub fn context(
    governance: &iroha_config::parameters::actual::Governance,
    asset_scale: u32,
) -> PlainVotingContextV1 {
    PlainVotingContextV1::Conviction(PlainConvictionPolicyV1 {
        asset_definition_id: governance.voting_asset_id.clone(),
        asset_scale,
        conviction_step_blocks: governance.conviction_step_blocks,
        max_conviction: governance.max_conviction,
        approval_threshold_numerator: governance.approval_threshold_q_num,
        approval_threshold_denominator: governance.approval_threshold_q_den,
        minimum_turnout: governance.min_turnout,
        minimum_bond: governance.min_bond_amount.clone(),
        bond_escrow_account: governance.bond_escrow_account.clone(),
        slash_receiver_account: governance.slash_receiver_account.clone(),
    })
}

/// Register absent fixture custody/asset state, then mint the exact requested voter balance.
///
/// Existing voter balances are left intact. Escrow is never precredited by this helper.
///
/// # Panics
/// Panics when actual Register/Mint execution rejects this explicit test setup.
pub fn fund_voter(
    transaction: &mut StateTransaction<'_, '_>,
    voter: &AccountId,
    balance: Quantity,
    scale: u32,
) {
    let governance = transaction.gov.clone();
    for account in [
        voter,
        &governance.bond_escrow_account,
        &governance.slash_receiver_account,
    ] {
        if transaction.world.accounts().get(account).is_none() {
            Register::account(Account::new(account.clone()))
                .execute(voter, transaction)
                .expect("register explicit public-ballot fixture account");
        }
    }
    if transaction
        .world
        .asset_definitions()
        .get(&governance.voting_asset_id)
        .is_none()
    {
        Register::asset_definition(AssetDefinition::new(
            governance.voting_asset_id.clone(),
            "public-ballot-fixture",
            NumericSpec::fractional(scale),
            AssetBalancePolicy::Global,
            None,
        ))
        .execute(voter, transaction)
        .expect("register explicit voting asset");
    }
    let asset = AssetId::new(governance.voting_asset_id, voter.clone());
    if transaction.world.assets().get(&asset).is_none() && !balance.is_zero() {
        Mint::asset_quantity(balance, asset)
            .execute(voter, transaction)
            .expect("mint explicit public-ballot fixture balance");
    }
}
