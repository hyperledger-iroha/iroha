//! Real-XOR custody of one finality-owned consensus slash on actual peers.
//!
//! The signed staking policy names a slash sink distinct from the stake escrow and a
//! partial slash fraction, so a committed penalty is an observable XOR transfer. Custody is
//! read from one committed tip; burned transaction fees are reconstructed from the
//! independently authenticated native chain, never from a node's own accounting.

use super::*;
use iroha::data_model::{
    block::consensus::{NexusFeeSettlementV1, PublicLaneStakingPolicy},
    nexus::{
        PublicLanePreparationOperationV1, PublicLanePreparationRequestV1, PublicLanePrepareBondV1,
        PublicLanePrepareClaimV1,
    },
};
use iroha_test_samples::{BOB_ID, CARPENTER_ID};

/// Signed slash fraction of the qualification networks: one tenth of the exposure.
pub const SLASH_BPS: u16 = 1_000;

/// The original staking policy every qualification genesis signs.
///
/// Stake escrow and slash sink are two existing, separately owned genesis accounts, so a
/// consensus slash moves XOR between distinct custody balances instead of relabelling one.
pub fn signed_policy(xor: &AssetDefinitionId) -> PublicLaneStakingPolicy {
    let mut policy =
        iroha_genesis::default_staking_policy(xor.clone(), CARPENTER_ID.clone(), BOB_ID.clone());
    policy.max_slash_bps = SLASH_BPS;
    policy
}

/// The exact slash of `exposure` under the signed fraction.
pub fn slash_of(exposure: u64) -> Result<Quantity> {
    let slashed = u128::from(exposure) * u128::from(SLASH_BPS);
    ensure!(
        slashed % 10_000 == 0,
        "qualification exposure must slash to a whole quantity"
    );
    Ok(Quantity::from(u64::try_from(slashed / 10_000)?))
}

/// Escrow, sink and supply of the staking currency at one committed tip.
pub struct Custody {
    /// Committed height every value was read at.
    pub height: u64,
    escrow_balance: Quantity,
    stake_reserved: Quantity,
    sink_balance: Quantity,
    supply: Quantity,
}

/// Read stake custody around one unchanged committed tip.
///
/// `probe` is any registered validator that is not being slashed: preparing a one-unit bond
/// to it is a read that returns the shared escrow's balance and stake reserve.
pub async fn custody(
    admin: &Client,
    policy: &PublicLaneStakingPolicy,
    escrow: &AssetId,
    probe: &AccountId,
) -> Result<Custody> {
    ensure!(
        escrow.account() == &policy.stake_escrow_account_id
            && escrow.definition() == &policy.stake_asset_id
            && policy.slash_sink_account_id != policy.stake_escrow_account_id,
        "slash custody requires the signed escrow and a distinct signed sink"
    );
    let sink = AssetId::with_scope(
        policy.stake_asset_id.clone(),
        policy.slash_sink_account_id.clone(),
        *escrow.scope(),
    );
    let request = PublicLanePreparationRequestV1 {
        lane_id: LaneId::SINGLE,
        valid_for_blocks: EPOCH,
        operation: PublicLanePreparationOperationV1::Bond(PublicLanePrepareBondV1 {
            validator: probe.clone(),
            staker: ALICE_ID.clone(),
            amount: 1_u64.into(),
        }),
    };
    let deadline = Instant::now() + WAIT;
    let prepare = || async {
        loop {
            let result = tokio::time::timeout_at(
                deadline.into(),
                admin.client().nexus().prepare_public_lane_plan(&request),
            )
            .await
            .wrap_err("slash custody preparation exceeded its read deadline")?;
            match result {
                Ok(prepared) => return Ok::<_, eyre::Report>(prepared),
                Err(error) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    let Some(delay) = committee_read_retry_delay(&error, remaining) else {
                        return Err(error.into());
                    };
                    sleep(delay).await;
                }
            }
        }
    };
    loop {
        ensure!(
            Instant::now() < deadline,
            "slash custody observation deadline elapsed"
        );
        let before = prepare().await?;
        let (sink_balance, supply) = read_on_dedicated_thread({
            let admin = admin.clone();
            let sink = sink.clone();
            let definition = policy.stake_asset_id.clone();
            move || -> Result<(Quantity, Quantity)> {
                let client = admin.client().with_request_deadline(deadline);
                Ok((
                    client
                        .query_single(FindAssetById::new(sink))?
                        .value()
                        .clone(),
                    client
                        .query_single(FindAssetDefinitionById::new(definition))?
                        .total_quantity()
                        .clone(),
                ))
            }
        })
        .await
        .wrap_err("slash sink and supply worker failed")?;
        let after = prepare().await?;
        if before.observed_height != after.observed_height
            || before.observed_block_hash != after.observed_block_hash
        {
            continue;
        }
        ensure!(
            before == after,
            "one committed tip returned inconsistent stake custody"
        );
        let row = before
            .balances
            .iter()
            .find(|row| &row.asset == escrow)
            .ok_or_else(|| eyre!("stake custody observation omitted the signed escrow"))?;
        ensure!(
            row.balance >= row.stake_reserved.checked_add(&row.rewards_reserved)?,
            "signed escrow does not cover its stake and reward reserves"
        );
        return Ok(Custody {
            height: before.observed_height,
            escrow_balance: row.balance.clone(),
            stake_reserved: row.stake_reserved.clone(),
            sink_balance,
            supply,
        });
    }
}

/// Require that `recipient` has no claimable staking reward on this network.
///
/// A reward exists only as funded conversion of collected retail fees under a
/// Parliament-enacted payout lifecycle and pricing policy. A fresh network has neither:
/// pricing activates no earlier than thirty days after its enactment block, on a Honiara
/// month boundary, and only a closed earning month converts. Service, stake, a slash or a
/// withdrawal must therefore never produce a claim plan here.
pub async fn assert_no_reward_entitlement(admin: &Client, recipient: &AccountId) -> Result<()> {
    let parameters = read_on_dedicated_thread({
        let admin = admin.clone();
        move || Ok(admin.client().query_single(FindParameters)?)
    })
    .await
    .wrap_err("validation-fee registry worker failed")?;
    ensure!(
        !parameters
            .custom()
            .contains_key(&ValidationFeePolicyRegistryV1::parameter_id()),
        "a fresh qualification network cannot hold an enacted validation-fee policy"
    );
    let request = PublicLanePreparationRequestV1 {
        lane_id: LaneId::SINGLE,
        valid_for_blocks: EPOCH,
        operation: PublicLanePreparationOperationV1::ClaimRewards(PublicLanePrepareClaimV1 {
            recipient: recipient.clone(),
        }),
    };
    let deadline = Instant::now() + WAIT;
    loop {
        let result = tokio::time::timeout_at(
            deadline.into(),
            admin.client().nexus().prepare_public_lane_plan(&request),
        )
        .await
        .wrap_err("reward claim preparation exceeded its read deadline")?;
        match result {
            Ok(_) => {
                return Err(eyre!(
                    "a network without funded fee conversion prepared a staking reward claim"
                ));
            }
            Err(error) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if let Some(delay) = committee_read_retry_delay(&error, remaining) {
                    sleep(delay).await;
                    continue;
                }
                // The node answers a refused preparation with its typed conflict, which the
                // client keeps: the stable refusal code and the deterministic reason
                // execution would reject with. A node fault carries neither and never
                // qualifies as a refusal.
                const REASON: &str = "no funded reward entitlement is available to claim";
                let Some(refusal) = error.api_error() else {
                    return Err(eyre!(
                        "reward claim preparation failed without a node refusal: {error:#}"
                    ));
                };
                ensure!(
                    refusal.status() == 409
                        && refusal.code() == iroha_torii_shared::STAKING_PREPARATION_REFUSED_CODE
                        && refusal.message().ends_with(REASON),
                    "reward claim preparation was refused for a different reason: {error:#}"
                );
                return Ok(());
            }
        }
    }
}

/// Sum the XOR actually burned as transaction fees after `start` in an authenticated chain.
fn burned_fees(chain: &[CertifiedBlock], start: u64, xor: &AssetDefinitionId) -> Result<Quantity> {
    let mut burned = Quantity::zero();
    for block in chain.iter().filter(|block| block.height() > start) {
        for index in 0..block.block().network_entrypoint_count() {
            let Some((_, output)) = block.block().network_output_at(u32::try_from(index)?) else {
                continue;
            };
            let Some(receipt) = output.result.nexus_fee_receipt() else {
                continue;
            };
            ensure!(
                receipt.fee_asset_id == *xor
                    && receipt.block_height == block.height()
                    && matches!(receipt.settlement, NexusFeeSettlementV1::Burn),
                "paid progress settled a fee outside the canonical XOR burn"
            );
            burned = burned.checked_add(&receipt.fee_amount)?;
        }
    }
    Ok(burned)
}

/// Require exactly one consensus slash of `slashed` between two custody observations.
///
/// `chain` is the independently authenticated native chain through `after.height`. The
/// escrow loses the slashed principal and its stake reserve, the distinct sink receives the
/// same quantity, and the currency supply changes only by the authenticated fee burns.
pub fn assert_slashed(
    before: &Custody,
    after: &Custody,
    chain: &[CertifiedBlock],
    xor: &AssetDefinitionId,
    slashed: &Quantity,
) -> Result<()> {
    ensure!(
        before.height < after.height
            && chain.last().is_some_and(|tip| tip.height() == after.height)
            && !slashed.is_zero(),
        "slash accounting requires a positive penalty over one authenticated interval"
    );
    let burned = burned_fees(chain, before.height, xor)?;
    ensure!(
        before.escrow_balance.checked_sub(slashed)? == after.escrow_balance
            && before.stake_reserved.checked_sub(slashed)? == after.stake_reserved
            && before.sink_balance.checked_add(slashed)? == after.sink_balance
            && before.supply.checked_sub(&burned)? == after.supply,
        "consensus slash did not move exactly the signed fraction from stake escrow to the signed sink"
    );
    Ok(())
}

/// Four bonded genesis voters and one separately running, never-admitted candidate process.
///
/// One voter's operator double-signs the next height. The honest daemons retain the
/// conflict, an honest leader admits it with ordinary paid work, and after the signed delay
/// the mandatory penalty moves one tenth of the offender's real XOR bond to the signed sink.
/// The slashed seat keeps voting: an obligated seat leaves only by certified replacement.
#[cfg(unix)]
pub async fn run_double_sign_qualification() -> Result<()> {
    const SEED: &str = "npos-double-sign-slash";
    const BOND: u64 = 1_000;
    init_instruction_registry();
    let npos = SumeragiNposParameters {
        epoch_length_blocks: NonZeroU64::new(EPOCH).expect("nonzero epoch"),
        min_self_bond: BOND.into(),
        evidence_horizon_blocks: EPOCH * 2,
        slashing_delay_blocks: SLASHING_DELAY,
        ..SumeragiNposParameters::default()
    };
    let xor: AssetDefinitionId = npos.xor_asset_definition_id.clone();
    ensure!(
        xor.to_string() == TAIRA_XOR,
        "staking must use real genesis-pinned XOR"
    );
    npos.validate().map_err(|error| eyre!(error))?;
    let policy = signed_policy(&xor);
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_max_validator_capacity(7)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .with_npos_genesis_bootstrap(BOND.into())
        .with_genesis_staking_policy(policy.clone())
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::EpochLengthBlocks(NonZeroU64::new(EPOCH).expect("nonzero epoch")),
        )))
        .with_committee_validator_p2p_bootstrap(CommitteeValidatorP2pBootstrap::new(1)?)?
        .with_config_layer(|layer| {
            layer.write(["logger", "filter"], "info,iroha_core::sumeragi=debug");
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )));
    let network = sandbox::build_network_or_skip(builder, SEED).ok_or_else(|| {
        eyre!("double-sign qualification requires an actual five-process disposable network")
    })?;
    let result = Box::pin(async {
        network.start_all().await?;
        network.ensure_blocks(1).await?;
        let admin = rebind_blocking_client(&network.client(), |builder| {
            builder.transaction_status_timeout = WAIT;
        });
        let genesis_hash = network.genesis().0.hash();
        let root_scope = iroha_genesis::signed_genesis_consensus_metadata(&network.genesis().0)?
            .sumeragi_context
            .root_scope;
        let network_id = network.network_id();
        let escrow = validator_xor_escrow(&network.genesis(), &xor)?;
        let voters = network
            .validators()
            .iter()
            .map(iroha_test_network::NetworkPeer::id)
            .collect::<Vec<_>>();
        let voter_set = voters.iter().cloned().collect::<BTreeSet<_>>();
        let carrier = network
            .committee_validators()
            .first()
            .ok_or_else(|| eyre!("fixture has no separately running non-voting process"))?
            .id();
        let offender_process = network
            .validators()
            .last()
            .ok_or_else(|| eyre!("fixture has no voters"))?;
        let offender = offender_process.id();
        let probe = network.validators()[0].account_id();
        ensure!(
            probe != offender_process.account_id() && !voter_set.contains(&carrier),
            "fixture must separate the offender, the custody probe and the transport process"
        );
        let authenticated = |height: u64| {
            let admin = admin.clone();
            async move {
                read_on_dedicated_thread(move || {
                    read_contiguous_finality_chain(
                        &admin,
                        network_id,
                        genesis_hash,
                        height,
                        root_scope,
                    )
                })
                .await
                .wrap_err("slash finality worker failed")
                .map(|(_, chain)| chain)
            }
        };

        // Ordinary paid progress first: the offence must be judged by committed history.
        advance_to_height(&network, &voters, 3).await?;
        let before = custody(&admin, &policy, &escrow, &probe).await?;
        let chain = authenticated(before.height).await?;
        let tip = chain
            .last()
            .ok_or_else(|| eyre!("no authenticated tip before the offence"))?;
        verify_equal_vote_context(tip, &voter_set)?;

        let sign = committee_equivocation::double_sign(&network, &carrier, &offender, tip).await?;
        ensure!(
            sign.height == before.height + 1 && sign.offender == offender,
            "double-sign must name the first uncommitted height of the observed custody"
        );
        // The custody probe's own node, an honest voter, reads the committed evidence.
        let penalty = committee_equivocation::await_applied_penalty(
            &network,
            &voters,
            &network.validators()[0],
            &sign,
            SLASHING_DELAY,
        )
        .await?;
        let after = custody(&admin, &policy, &escrow, &probe).await?;
        ensure!(
            after.height == penalty.applied_height
                && penalty.applied_height == penalty.admitted_height + SLASHING_DELAY,
            "custody must be observed at the exact height that applied the penalty"
        );
        let chain = authenticated(after.height).await?;
        assert_slashed(&before, &after, &chain, &xor, &slash_of(BOND)?)?;
        for block in chain.iter().filter(|block| block.height() > before.height) {
            verify_equal_vote_context(block, &voter_set)?;
        }
        assert_no_reward_entitlement(&admin, &offender_process.account_id()).await?;

        // The slashed seat is still obligated: the same exact quorum keeps finalizing paid
        // work, and no further penalty follows the single committed offence.
        advance_to_height(&network, &voters, after.height + 2).await?;
        let settled = custody(&admin, &policy, &escrow, &probe).await?;
        let chain = authenticated(settled.height).await?;
        ensure!(
            settled.height == after.height + 2
                && settled.escrow_balance == after.escrow_balance
                && settled.stake_reserved == after.stake_reserved
                && settled.sink_balance == after.sink_balance
                && after
                    .supply
                    .checked_sub(&burned_fees(&chain, after.height, &xor)?)?
                    == settled.supply,
            "one committed offence was penalized more than once"
        );
        for block in chain.iter().filter(|block| block.height() > after.height) {
            verify_equal_vote_context(block, &voter_set)?;
        }
        Ok(())
    })
    .await;
    let closure = shutdown_all_cleanly(&network).await;
    Box::pin(network.shutdown_and_release()).await;
    match (result, closure) {
        (Ok(()), closure) => closure,
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(closure)) => Err(error).wrap_err(format!(
            "validator process closure also failed: {closure:#}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_policy_separates_escrow_from_sink_and_slashes_one_tenth() -> Result<()> {
        let xor: AssetDefinitionId = TAIRA_XOR.parse()?;
        let policy = signed_policy(&xor);
        policy.validate().map_err(|error| eyre!("{error}"))?;
        ensure!(
            policy.stake_asset_id == xor
                && policy.stake_escrow_account_id != policy.slash_sink_account_id
                && policy.max_slash_bps == SLASH_BPS,
            "qualification policy lost its distinct sink or partial slash"
        );
        assert_eq!(slash_of(1_000)?, Quantity::from(100_u64));
        assert_eq!(slash_of(2_000)?, Quantity::from(200_u64));
        assert!(slash_of(1_005).is_err());
        Ok(())
    }

    #[test]
    fn slash_accounting_requires_exact_escrow_sink_and_supply_movement() -> Result<()> {
        let observed = |height, escrow: u64, reserved: u64, sink: u64, supply: u64| Custody {
            height,
            escrow_balance: escrow.into(),
            stake_reserved: reserved.into(),
            sink_balance: sink.into(),
            supply: supply.into(),
        };
        let xor: AssetDefinitionId = TAIRA_XOR.parse()?;
        let before = observed(3, 4_000, 4_000, 10, 9_000);
        // An interval without an authenticated tip at the observed height is refused
        // before any balance is compared.
        assert!(
            assert_slashed(
                &before,
                &observed(5, 3_900, 3_900, 110, 9_000),
                &[],
                &xor,
                &100_u64.into()
            )
            .is_err()
        );
        assert!(
            assert_slashed(
                &before,
                &observed(3, 3_900, 3_900, 110, 9_000),
                &[],
                &xor,
                &100_u64.into()
            )
            .is_err()
        );
        assert!(
            assert_slashed(
                &before,
                &observed(5, 4_000, 4_000, 10, 9_000),
                &[],
                &xor,
                &Quantity::zero()
            )
            .is_err()
        );
        Ok(())
    }
}
