//! Startup SCCP liability floor against the genesis-pinned staking policy.

use iroha_core::{smartcontracts::isi::sccp::params, state::WorldReadOnly};

/// Refuse an unsafe current or retained liability window before startup.
/// The exact committed monetary policy is borrowed from the original World cut.
pub fn validate(world: &(impl WorldReadOnly + ?Sized)) -> Result<(), params::SccpPolicyError> {
    params::validate_committed_staking_policy(world)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::isi::sccp::store,
        state::{State, World},
    };
    use iroha_data_model::parameter::Parameter;
    use iroha_data_model::{block::BlockHeader, sccp::params::SccpParametersV1};
    use std::num::NonZeroU64;

    #[test]
    fn absent_sccp_needs_no_policy_but_disabled_sccp_keeps_original_liability_floor() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let mut block = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
        let mut stx = block.transaction();
        validate(&*stx.world).unwrap();
        let mut parameters = SccpParametersV1::taira_default();
        parameters.enabled = false;
        store::parameters::set(&mut stx, Some(parameters));
        assert_eq!(
            validate(&*stx.world),
            Err(params::SccpPolicyError::MissingStakingPolicy)
        );
        let asset = iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .unwrap();
        let custody = iroha_config::parameters::defaults::governance::bond_escrow_account_id();
        let mut staking = iroha_genesis::default_staking_policy(asset, custody.clone(), custody);
        staking.unbonding_delay_ms = 0;
        staking.max_slash_bps = 0;
        stx.world
            .set_parameter_for_testing(Parameter::Custom(
                staking.clone().into_custom_parameter().unwrap(),
            ))
            .unwrap();
        assert_eq!(
            validate(&*stx.world),
            Err(params::SccpPolicyError::ZeroSlashRate)
        );
        staking.max_slash_bps = 1;
        stx.world
            .set_parameter_for_testing(Parameter::Custom(
                staking.clone().into_custom_parameter().unwrap(),
            ))
            .unwrap();
        assert_eq!(
            validate(&*stx.world),
            Err(params::SccpPolicyError::Parameters(
                iroha_data_model::sccp::params::SccpParametersError::UnbondingDelayTooShort
            ))
        );
        staking.unbonding_delay_ms =
            parameters.committee_validity_ms + parameters.forgery_evidence_grace_ms;
        stx.world
            .set_parameter_for_testing(Parameter::Custom(staking.into_custom_parameter().unwrap()))
            .unwrap();
        validate(&*stx.world).unwrap();
    }
}
