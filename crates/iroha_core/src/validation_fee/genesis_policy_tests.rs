//! Payout fixtures author monetary policy before authenticating their original genesis.

use super::*;
use crate::{
    state::{StateReadOnly, WorldReadOnly},
    sumeragi::test_chain::CertifiedTestChain,
};
use iroha_data_model::{
    block::consensus::PublicLaneStakingPolicy,
    isi::SetParameter,
    parameter::{Parameter, system::SumeragiNposParameters},
};

#[test]
fn payout_policy_is_signed_before_genesis_and_committed_before_parent_penalties() {
    let config = validation_fee_payout_config(&account(55), 1_000);
    assert!(
        config
            .world
            .view()
            .sumeragi_npos_parameters()
            .unwrap()
            .is_none()
    );
    assert!(config.world.view().parameters().staking_policy().is_none());
    assert_eq!(
        config.consensus_mode,
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos
    );
    let authored = config.genesis_parameters.clone();
    let prepared = CertifiedTestChain::prepare(config).expect("original payout genesis prepares");
    assert!(
        prepared
            .state
            .view()
            .world()
            .sumeragi_npos_parameters()
            .unwrap()
            .is_none(),
        "preparation must not install policy into the original parent State"
    );
    assert!(
        prepared
            .state
            .view()
            .world()
            .parameters()
            .staking_policy()
            .is_none()
    );
    let signed = prepared
        .genesis
        .block()
        .external_transactions()
        .flat_map(|transaction| {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                panic!("genesis contains original instruction batches");
            };
            instructions.iter()
        })
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        .map(|set| set.inner().clone())
        .collect::<Vec<_>>();
    for parameter in &authored {
        assert_eq!(
            signed.iter().filter(|signed| *signed == parameter).count(),
            1
        );
    }
    let staking = authored
        .iter()
        .find_map(|parameter| match parameter {
            Parameter::Custom(custom)
                if custom.id() == &PublicLaneStakingPolicy::parameter_id() =>
            {
                Some(PublicLaneStakingPolicy::from_custom_parameter(custom).unwrap())
            }
            _ => None,
        })
        .expect("one explicit signed staking policy");
    let mut chain =
        CertifiedTestChain::from_prepared(prepared).expect("original payout genesis applies");
    {
        let view = chain.state().view();
        assert_eq!(view.height(), 1);
        assert_eq!(view.world().parameters().staking_policy(), Some(&staking));
        assert_eq!(
            view.world().sumeragi_npos_parameters().unwrap(),
            Some(SumeragiNposParameters::default())
        );
        crate::state::validate_network_xor_asset(view.world(), &xor_asset()).unwrap();
    }
    // This adds an actual signed clock transaction. Its penalty preparation reads
    // the now-committed parent NPoS and staking policies rather than fixture defaults.
    chain.commit_at(2_000, Vec::new());
    assert_eq!(chain.height(), 2);
    assert_eq!(
        chain.state().view().world().parameters().staking_policy(),
        Some(&staking)
    );
}

#[test]
fn payout_authoring_still_refuses_missing_and_duplicate_signed_staking() {
    let mut missing = validation_fee_payout_config(&account(55), 1_000);
    missing.genesis_parameters.retain(|parameter| {
        !matches!(parameter, Parameter::Custom(custom)
            if custom.id() == &PublicLaneStakingPolicy::parameter_id())
    });
    let error = CertifiedTestChain::prepare(missing)
        .err()
        .expect("missing signed staking fails");
    assert!(
        error
            .error
            .to_string()
            .contains("missing committed public-lane staking policy")
    );
    let mut duplicate = validation_fee_payout_config(&account(55), 1_000);
    let staking = duplicate
        .genesis_parameters
        .iter()
        .find(|parameter| {
            matches!(parameter, Parameter::Custom(custom)
            if custom.id() == &PublicLaneStakingPolicy::parameter_id())
        })
        .unwrap()
        .clone();
    duplicate.genesis_parameters.push(staking);
    let error = CertifiedTestChain::prepare(duplicate)
        .err()
        .expect("equal duplicate signed staking fails");
    assert!(
        error
            .error
            .to_string()
            .contains("repeated public_lane_staking_v1")
    );
}

#[test]
fn payout_preseeded_npos_does_not_substitute_for_committed_parent_staking() {
    let config = validation_fee_payout_config(&account(55), 1_000);
    {
        let mut parameters = crate::state::parameter_cell_block_for_testing(&config.world).unwrap();
        crate::state::set_parameter_cell_for_testing(
            &mut parameters,
            Parameter::Custom(SumeragiNposParameters::default().into_custom_parameter()),
            config.world.sccp_committee_generations.allocation_budget(),
        )
        .unwrap();
        parameters.commit();
    }
    assert!(config.world.view().parameters().staking_policy().is_none());
    let error = CertifiedTestChain::prepare(config)
        .err()
        .expect("uncommitted parent policy is not signed authority");
    assert!(
        error
            .error
            .to_string()
            .contains("native penalty preparation: missing committed public-lane staking policy")
    );
}
