//! SCCP v1 consensus parameters (`specs/sccp.md` §4.1). Owner: ws31.
//!
//! `SccpParametersV1` lives in dedicated world state written only by the genesis-only
//! `InitializeSccpV1` and the Parliament-enacted `SetParameters`. SCCP exists on a network iff
//! the parameters are present. `enabled = false` stops only value-moving effects.

use super::store;
use crate::state::WorldReadOnly;
use iroha_data_model::NetworkId;
use iroha_data_model::block::consensus::PublicLaneStakingPolicy;
use iroha_data_model::sccp::params::{
    SCCP_MIN_EPOCH_MS_V1, SccpParameterContextV1, SccpParametersError, SccpParametersV1,
};
use mv::storage::StorageReadOnly;

/// A deterministic violation of SCCP's epoch or genesis-pinned staking policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SccpPolicyError {
    /// An intrinsic parameter or current liability-window bound failed.
    #[error(transparent)]
    Parameters(#[from] SccpParametersError),
    /// An existing SCCP scope has no original committed monetary policy.
    #[error("SCCP requires an original committed public_lane_staking_v1")]
    MissingStakingPolicy,
    /// The retained genesis or predecessor binding does not match the original cut.
    #[error("SCCP staking monetary authority does not match the original State source")]
    OriginalStakingSourceMismatch,
    /// Retained liabilities cannot survive removal of the SCCP parameter owner.
    #[error("retained SCCP generations require the original SCCP parameters")]
    MissingSccpParameters,
    /// A slashable committee requires a nonzero pinned slash rate.
    #[error("SCCP requires committed staking max_slash_bps >= 1")]
    ZeroSlashRate,
    /// A staking slash moves value under protocol custody, which the escrow guards skip,
    /// so neither of its accounts may be an SCCP route escrow (§4.15).
    #[error("the staking {0} cannot be an SCCP route escrow")]
    EscrowStakingAccount(&'static str),
    /// An already retained generation remains liable beyond the proposed staking delay.
    #[error(
        "SCCP generation {generation} validity {validity_ms} + grace {grace_ms} exceeds unbonding delay {unbonding_delay_ms}"
    )]
    RetainedLiability {
        /// Original generation identifier.
        generation: u64,
        /// Original snapshotted validity interval.
        validity_ms: u64,
        /// Original snapshotted evidence grace.
        grace_ms: u64,
        /// Genesis-pinned staking delay.
        unbonding_delay_ms: u64,
    },
    /// The effective epoch is shorter than the one-hour SCCP floor.
    #[error(
        "SCCP requires epoch_length_blocks {blocks} * block_cadence_ms {cadence_ms} >= {SCCP_MIN_EPOCH_MS_V1}"
    )]
    EpochTooShort {
        /// Prospective epoch length.
        blocks: u64,
        /// Signed-genesis block cadence.
        cadence_ms: u64,
    },
    /// Epoch duration arithmetic cannot be represented by the protocol clock.
    #[error("SCCP epoch duration overflows: {blocks} * {cadence_ms}")]
    EpochOverflow {
        /// Prospective epoch length.
        blocks: u64,
        /// Signed-genesis block cadence.
        cadence_ms: u64,
    },
}

/// Validate the §4.2 epoch floor using checked protocol-clock arithmetic.
///
/// # Errors
/// Returns the exact too-short or overflowing epoch operands.
pub fn validate_epoch(blocks: u64, cadence_ms: u64) -> Result<(), SccpPolicyError> {
    let duration = blocks
        .checked_mul(cadence_ms)
        .ok_or(SccpPolicyError::EpochOverflow { blocks, cadence_ms })?;
    if duration < SCCP_MIN_EPOCH_MS_V1 {
        return Err(SccpPolicyError::EpochTooShort { blocks, cadence_ms });
    }
    Ok(())
}

/// Validate proposed parameters and every retained generation against pinned staking (§4.8 L1).
/// This also applies when SCCP's value-moving switch is disabled.
///
/// # Errors
/// Returns the first invalid current parameter, pinned staking value or retained liability.
pub fn validate_staking_policy(
    world: &(impl WorldReadOnly + ?Sized),
    parameters: &SccpParametersV1,
    staking: &PublicLaneStakingPolicy,
) -> Result<(), SccpPolicyError> {
    let unbonding_delay_ms = staking.unbonding_delay_ms;
    if staking.max_slash_bps == 0 {
        return Err(SccpPolicyError::ZeroSlashRate);
    }
    parameters.validate_against(&SccpParameterContextV1 {
        // TODO: WP-S5 supplies the authenticated Parliament attempt latency at launch.
        parliament_sccp_attempt_latency_ms: None,
        unbonding_delay_ms,
    })?;
    for (generation, owner) in world.sccp_committee_generations().iter() {
        if owner
            .validity_ms
            .checked_add(owner.grace_ms)
            .is_none_or(|liability| liability > unbonding_delay_ms)
        {
            return Err(SccpPolicyError::RetainedLiability {
                generation: *generation,
                validity_ms: owner.validity_ms,
                grace_ms: owner.grace_ms,
                unbonding_delay_ms,
            });
        }
    }
    Ok(())
}

/// Validate a proposed original monetary successor against current SCCP policy
/// and every retained generation. A disabled value-moving switch keeps liabilities.
///
/// This is also called by monetary parameter preparation before any MV mutation;
/// the caller supplies the candidate, not a node-local projection or default.
///
/// # Errors
/// Returns the exact current or retained liability violation. Orphaned retained
/// generations are invalid even if the SCCP parameter cell is absent.
pub fn validate_staking_successor(
    world: &(impl WorldReadOnly + ?Sized),
    staking: &PublicLaneStakingPolicy,
) -> Result<(), SccpPolicyError> {
    match parameters(world) {
        Some(parameters) => validate_staking_policy(world, &parameters, staking),
        None if world.sccp_committee_generations().iter().next().is_none() => Ok(()),
        None => Err(SccpPolicyError::MissingSccpParameters),
    }
}

/// Refuse a staking policy whose slash sink or stake escrow is an SCCP route escrow of
/// `network_id` (§4.15), whether or not SCCP is initialized yet. A slash credits the sink and
/// debits the stake escrow under protocol custody, which skips the escrow guards, so either
/// account would move escrow value outside SCCP and break `balance = Σ liability + stranded`.
///
/// # Errors
/// Names the first staking account that is a route escrow id.
pub fn validate_staking_custody(
    network_id: &NetworkId,
    staking: &PublicLaneStakingPolicy,
) -> Result<(), SccpPolicyError> {
    for (role, account) in [
        ("slash sink", &staking.slash_sink_account_id),
        ("stake escrow", &staking.stake_escrow_account_id),
    ] {
        if super::escrow::is_route_escrow_id(network_id, account) {
            return Err(SccpPolicyError::EscrowStakingAccount(role));
        }
    }
    Ok(())
}

/// Validate the exact committed policy from one complete original World cut.
/// This is the startup/restore boundary; node configuration is not an input.
///
/// # Errors
/// Missing original monetary authority and unsafe retained liabilities fail closed.
pub fn validate_committed_staking_policy(
    world: &(impl WorldReadOnly + ?Sized),
) -> Result<(), SccpPolicyError> {
    if parameters(world).is_none() && world.sccp_committee_generations().iter().next().is_none() {
        return Ok(());
    }
    let staking = world
        .parameters()
        .staking_policy()
        .ok_or(SccpPolicyError::MissingStakingPolicy)?;
    validate_staking_successor(world, staking)
}

/// Return the SCCP parameters, or `None` when SCCP does not exist on this network.
#[must_use]
pub fn parameters(world: &(impl WorldReadOnly + ?Sized)) -> Option<SccpParametersV1> {
    *store::parameters::get(world)
}

/// Return whether SCCP exists on this network (its parameters are present).
#[must_use]
pub fn exists(world: &(impl WorldReadOnly + ?Sized)) -> bool {
    store::parameters::get(world).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};

    #[test]
    fn absent_parameters_mean_sccp_does_not_exist() {
        let state = blank_state();
        let view = state.world_view();
        assert_eq!(parameters(&view), None);
        assert!(!exists(&view));
    }

    #[test]
    fn parameters_follow_the_stored_switch() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut stored = SccpParametersV1::taira_default();
        store::parameters::set(&mut stx, Some(stored));
        assert!(exists(&*stx.world));
        assert_eq!(parameters(&*stx.world), Some(stored));
        assert_eq!(parameters(&*stx.world).map(|p| p.enabled), Some(true));
        stored.enabled = false;
        store::parameters::set(&mut stx, Some(stored));
        assert!(exists(&*stx.world));
        assert_eq!(parameters(&*stx.world).map(|p| p.enabled), Some(false));
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;
    use crate::state::World;
    fn staking_policy(delay: u64, slash: u16) -> PublicLaneStakingPolicy {
        let asset = iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .unwrap();
        let mut policy = crate::sumeragi::test_chain::staking_fixture_policy(asset);
        policy.unbonding_delay_ms = delay;
        policy.max_slash_bps = slash;
        policy
    }

    #[test]
    fn epoch_floor_exact_one_below_and_overflow_are_distinct() {
        validate_epoch(3600, 1000).unwrap();
        validate_epoch(3601, 1000).unwrap();
        assert_eq!(
            validate_epoch(3599, 1000),
            Err(SccpPolicyError::EpochTooShort {
                blocks: 3599,
                cadence_ms: 1000
            })
        );
        assert_eq!(
            validate_epoch(u64::MAX, 2),
            Err(SccpPolicyError::EpochOverflow {
                blocks: u64::MAX,
                cadence_ms: 2
            })
        );
    }

    #[test]
    fn committed_staking_exact_delay_zero_slash_and_full_clock_range() {
        let world = World::default();
        let world = world.view();
        let parameters = SccpParametersV1::taira_default();
        let exact = parameters.committee_validity_ms + parameters.forgery_evidence_grace_ms;
        let mut staking = staking_policy(exact, 1);
        validate_staking_policy(&world, &parameters, &staking).unwrap();
        staking.unbonding_delay_ms = exact - 1;
        assert_eq!(
            validate_staking_policy(&world, &parameters, &staking),
            Err(SccpPolicyError::Parameters(
                SccpParametersError::UnbondingDelayTooShort
            ))
        );
        staking.unbonding_delay_ms = exact;
        staking.max_slash_bps = 0;
        assert_eq!(
            validate_staking_policy(&world, &parameters, &staking),
            Err(SccpPolicyError::ZeroSlashRate)
        );
        staking.max_slash_bps = 1;
        staking.unbonding_delay_ms = u64::MAX;
        validate_staking_policy(&world, &parameters, &staking).unwrap();
    }
    #[test]
    fn staking_custody_refuses_every_route_escrow_id_of_its_own_network() {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::{block::BlockHeader, sccp::registry::SCCP_ROUTE_NETWORKS_V1};
        let network = |seed: u8| {
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([seed; 32]),
            ))
        };
        let (own, foreign) = (network(1), network(2));
        let policy = staking_policy(u64::MAX, 1);
        validate_staking_custody(&own, &policy).unwrap();
        for route in SCCP_ROUTE_NETWORKS_V1 {
            let escrow = super::super::escrow::escrow_account(&own, route).unwrap();
            let mut sink = policy.clone();
            sink.slash_sink_account_id = escrow.clone();
            assert_eq!(
                validate_staking_custody(&own, &sink),
                Err(SccpPolicyError::EscrowStakingAccount("slash sink"))
            );
            let mut custody = policy.clone();
            custody.stake_escrow_account_id = escrow.clone();
            assert_eq!(
                validate_staking_custody(&own, &custody),
                Err(SccpPolicyError::EscrowStakingAccount("stake escrow"))
            );
            // Another network's escrow is an ordinary account here.
            validate_staking_custody(&foreign, &sink).unwrap();
            validate_staking_custody(&foreign, &custody).unwrap();
        }
    }

    #[test]
    fn disabled_sccp_requires_original_policy_and_checks_candidate_without_installing() {
        use iroha_data_model::parameter::Parameter;
        let mut world = World::default();
        validate_committed_staking_policy(&world.view()).unwrap();
        let mut parameters = SccpParametersV1::taira_default();
        parameters.enabled = false;
        {
            let mut block = world.block();
            *block.sccp_parameters.get_mut() = Some(parameters);
            block.commit();
        }
        assert_eq!(
            validate_committed_staking_policy(&world.view()),
            Err(SccpPolicyError::MissingStakingPolicy)
        );
        let exact = parameters.committee_validity_ms + parameters.forgery_evidence_grace_ms;
        let mut policy = staking_policy(exact, 1);
        world
            .set_parameter_for_testing(Parameter::Custom(
                policy.clone().into_custom_parameter().unwrap(),
            ))
            .unwrap();
        validate_committed_staking_policy(&world.view()).unwrap();
        let original = world.view().parameters().staking_policy().unwrap().clone();
        policy.unbonding_delay_ms -= 1;
        assert_eq!(
            validate_staking_successor(&world.view(), &policy),
            Err(SccpPolicyError::Parameters(
                SccpParametersError::UnbondingDelayTooShort
            ))
        );
        assert_eq!(world.view().parameters().staking_policy(), Some(&original));
        validate_committed_staking_policy(&world.view()).unwrap();
    }

    #[test]
    fn retained_generation_cannot_be_underfunded_or_orphaned_by_successor() {
        use crate::smartcontracts::isi::sccp::generation_record::RetainedSccpGeneration;
        let world = crate::state::sccp_snapshot_state::tests::sccp_world();
        let budget = world.sccp_committee_generations.allocation_budget().clone();
        let parameters = parameters(&world.view()).unwrap();
        let delay = parameters.committee_validity_ms + parameters.forgery_evidence_grace_ms;
        let policy = staking_policy(delay, 1);
        let mut retained = (**world.sccp_committee_generations.view().get(&1).unwrap()).clone();
        retained.validity_ms = parameters.committee_validity_ms;
        retained.grace_ms = parameters.forgery_evidence_grace_ms + 1;
        retained.deadline_ms = retained.start_timestamp_ms + retained.validity_ms;
        retained.liable_until_lc = retained.start_lc + retained.validity_ms + retained.grace_ms;
        let retained = RetainedSccpGeneration::admit(&retained, &budget).unwrap();
        {
            let mut block = world.block();
            block
                .sccp_committee_generations
                .try_insert_admitted(1, retained)
                .unwrap();
            block.commit();
        }
        assert_eq!(
            validate_staking_successor(&world.view(), &policy),
            Err(SccpPolicyError::RetainedLiability {
                generation: 1,
                validity_ms: parameters.committee_validity_ms,
                grace_ms: parameters.forgery_evidence_grace_ms + 1,
                unbonding_delay_ms: delay
            })
        );
        {
            let mut block = world.block();
            *block.sccp_parameters.get_mut() = None;
            block.commit();
        }
        assert_eq!(
            validate_staking_successor(&world.view(), &policy),
            Err(SccpPolicyError::MissingSccpParameters)
        );
    }
}
