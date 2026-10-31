//! SCCP epoch-duration enforcement through the actual parameter instruction boundary.

use super::{
    store,
    test_support::{authority, blank_state, header},
};
use crate::smartcontracts::Execute;
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    isi::{
        SetParameter,
        error::{InstructionExecutionError, InvalidParameterError},
    },
    parameter::{
        CustomParameter, Parameter,
        system::{ConsensusHandshakeMetadata, SumeragiParameter},
    },
    sccp::params::SccpParametersV1,
};
use iroha_primitives::json::Json;
use std::num::NonZeroU64;

#[test]
fn sccp_refuses_short_epoch_before_mutating_consensus_parameters() {
    let state = blank_state();
    let mut block = state.block(header(1));
    let mut stx = block.transaction();
    store::parameters::set(&mut stx, Some(SccpParametersV1::taira_default()));
    let mut consensus = stx.world.parameters.get().sumeragi().clone();
    consensus.epoch_length_blocks = NonZeroU64::new(3_600).unwrap();
    consensus.block_cadence_ms = NonZeroU64::new(1_000).unwrap();
    stx.world
        .set_sumeragi_for_testing(consensus)
        .expect("explicit fixture consensus parameters");
    let before = stx.world.parameters.get().clone();
    let update = |epoch| {
        SetParameter::new(Parameter::Sumeragi(SumeragiParameter::EpochLengthBlocks(
            NonZeroU64::new(epoch).unwrap(),
        )))
    };
    let error = update(3_599).execute(&authority(1), &mut stx).unwrap_err();
    assert!(
        matches!(
            &error,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(reason))
                if reason.contains("SCCP requires epoch_length_blocks")
        ),
        "{error:?}"
    );
    assert_eq!(*stx.world.parameters.get(), before);
    update(3_600).execute(&authority(1), &mut stx).unwrap();
    assert_eq!(
        stx.world
            .parameters
            .get()
            .sumeragi()
            .epoch_length_blocks
            .get(),
        3_600
    );
}

#[test]
fn genesis_cadence_rewrite_cannot_break_an_already_initialized_sccp_epoch() {
    let state = blank_state();
    let mut block = state.block(header(1));
    let mut stx = block.transaction();
    store::parameters::set(&mut stx, Some(SccpParametersV1::taira_default()));
    let mut consensus = stx.world.parameters.get().sumeragi().clone();
    consensus.epoch_length_blocks = NonZeroU64::new(3_600).unwrap();
    consensus.block_cadence_ms = NonZeroU64::new(1_000).unwrap();
    stx.world
        .set_sumeragi_for_testing(consensus)
        .expect("explicit fixture consensus parameters");
    let Parameter::Custom(original) =
        crate::sumeragi::lanes::routing::test_support::metadata(SumeragiRootScope::Global)
    else {
        panic!("handshake metadata fixture");
    };
    let mut metadata: ConsensusHandshakeMetadata = original.payload().try_into_any().unwrap();
    metadata.block_cadence_ms = NonZeroU64::new(999).unwrap();
    let changed = CustomParameter::new(
        original.id().clone(),
        Json::from_norito_value_ref(&norito::json::value::to_value(&metadata).unwrap()).unwrap(),
    );
    let before = stx.world.parameters.get().clone();
    let error = SetParameter::new(Parameter::Custom(changed))
        .execute(&authority(1), &mut stx)
        .unwrap_err();
    assert!(
        matches!(
            &error,
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(reason))
                if reason.contains("SCCP requires epoch_length_blocks")
        ),
        "{error:?}"
    );
    assert_eq!(*stx.world.parameters.get(), before);
    SetParameter::new(Parameter::Custom(original))
        .execute(&authority(1), &mut stx)
        .unwrap();
    assert_eq!(
        stx.world.parameters.get().sumeragi().block_cadence_ms.get(),
        1_000
    );
}

#[test]
fn staking_policy_naming_a_route_escrow_is_refused_when_set_not_at_penalty_time() {
    use crate::state::StateReadOnly as _;
    use iroha_data_model::{bridge::SccpNetworkV1, parameter::system::SumeragiNposParameters};
    let mut original = crate::sumeragi::test_chain::staking_fixture_policy(
        SumeragiNposParameters::default().xor_asset_definition_id,
    );
    original.unbonding_delay_ms = super::test_support::SCCP_UNBONDING_DELAY_MS;
    original.max_slash_bps = 1;
    let state = super::test_support::blank_state_with_staking(Some(original.clone()));
    let mut block = state.block(header(2));
    let mut stx = block.transaction();
    // SCCP is not initialized: the escrow ids are fixed by the NetworkId alone.
    assert!(store::parameters::get(&*stx.world).is_none());
    let escrow =
        super::escrow::escrow_account(stx.network_id(), SccpNetworkV1::BscMainnet).unwrap();
    let before = stx.world.parameters.get().clone();
    let mut next = original;
    next.unbonding_delay_ms += 1;
    for role in ["slash sink", "stake escrow"] {
        let mut captured = next.clone();
        if role == "slash sink" {
            captured.slash_sink_account_id = escrow.clone();
        } else {
            captured.stake_escrow_account_id = escrow.clone();
        }
        let error = SetParameter::new(Parameter::Custom(captured.into_custom_parameter().unwrap()))
            .execute(&authority(1), &mut stx)
            .unwrap_err();
        assert!(
            matches!(
                &error,
                InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(reason))
                    if reason == &format!("the staking {role} cannot be an SCCP route escrow")
            ),
            "{error:?}"
        );
        assert_eq!(*stx.world.parameters.get(), before);
    }
    SetParameter::new(Parameter::Custom(
        next.clone().into_custom_parameter().unwrap(),
    ))
    .execute(&authority(1), &mut stx)
    .unwrap();
    assert_eq!(stx.world.parameters.get().staking_policy(), Some(&next));
}
