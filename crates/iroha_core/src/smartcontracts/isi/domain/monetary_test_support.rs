//! Explicit monetary author input for domain/account retirement component fixtures.
//! These helpers create no signed-genesis authority and read no node configuration.

use iroha_data_model::{
    asset::AssetDefinitionId,
    block::consensus::{PublicLaneStakingPolicy, RootFeeExemptions, RootFeePolicy},
    parameter::Parameter,
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::ALICE_ID;

/// Fixed currency chosen by the fixture author, outside the domains under retirement.
pub fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("monetary", "fixture").expect("fixture domain"),
        "coin".parse().expect("fixture asset"),
    )
}

/// Complete explicit fee recipe; a component zero schedule is intentional.
pub fn fees() -> RootFeePolicy {
    RootFeePolicy {
        fee_asset_id: asset(),
        fee_sink_account_id: ALICE_ID.clone(),
        sponsor_vault_custody_account_id: ALICE_ID.clone(),
        successful_claim_fee_exempt_authorities: RootFeeExemptions::new(),
        base_fee: Quantity::zero(),
        per_byte_fee: Quantity::zero(),
        per_instruction_fee: Quantity::zero(),
        per_gas_unit_fee: Quantity::zero(),
    }
}

/// Complete explicit staking recipe using the same authored currency and custody.
pub fn staking() -> PublicLaneStakingPolicy {
    let mut policy = crate::sumeragi::test_chain::staking_fixture_policy(asset());
    policy.stake_escrow_account_id = ALICE_ID.clone();
    policy.slash_sink_account_id = ALICE_ID.clone();
    policy
}

/// Install complete original policies in a mutable, pre-chain component World.
pub fn install(world: &mut crate::state::World) {
    world
        .set_parameter_for_testing(Parameter::Custom(
            fees().into_custom_parameter().expect("explicit fee recipe"),
        ))
        .expect("retain explicit original fee policy");
    world
        .set_parameter_for_testing(Parameter::Custom(
            staking()
                .into_custom_parameter()
                .expect("explicit staking recipe"),
        ))
        .expect("retain explicit original staking policy");
}
