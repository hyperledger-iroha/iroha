//! Funded real-XOR rewards and source-bound withdrawal across certified replacement.

use super::*;
use iroha::{
    client::{AccountTransactionDraft, FeeQuoteRequest},
    crypto::HashOf,
    data_model::{
        block::consensus::NexusFeeSettlementV1,
        isi::staking::{
            ClaimPublicLaneRewards, FinalizePublicLaneUnbond, RecordPublicLaneRewards,
            SchedulePublicLaneUnbond,
        },
        nexus::{
            FeeDebitSource, PublicLanePreparationBalanceV1, PublicLanePreparationOperationV1,
            PublicLanePreparationRequestV1, PublicLanePreparationV1, PublicLanePrepareBondV1,
            PublicLanePrepareClaimV1, PublicLanePrepareUnbondV1, PublicLanePreparedPlanV1,
            PublicLaneRewardRole, PublicLaneRewardShare, PublicLaneUnbonding,
            public_lane_unbonding_commitment,
        },
    },
};
use std::time::{SystemTime, UNIX_EPOCH};

const REWARD_EPOCH: u64 = 2;

/// Select a finite wall-clock unlock; eligibility still requires certified replacement
/// and the complete authenticated evidence/slashing horizon.
pub(super) fn finite_release_deadline() -> Result<u64> {
    let now = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    now.checked_add(60_000)
        .ok_or_else(|| eyre!("disposable release timestamp overflow"))
}

struct Observation {
    prepared: PublicLanePreparationV1,
    chain: Vec<CertifiedBlock>,
    supply: Quantity,
}

impl Observation {
    fn asset(&self, asset: &AssetId) -> Result<&PublicLanePreparationBalanceV1> {
        self.prepared
            .balances
            .iter()
            .find(|row| &row.asset == asset)
            .ok_or_else(|| eyre!("prepared plan omitted exact custody asset {asset}"))
    }
}

fn same_observed_tip(left: &PublicLanePreparationV1, right: &PublicLanePreparationV1) -> bool {
    left.observed_height == right.observed_height
        && left.observed_block_hash == right.observed_block_hash
}

/// Retained real stake and the exact obligation committed by its pending request.
pub(super) struct WithdrawalLifecycle {
    owner: Operator,
    escrow: AssetId,
    destination: AssetId,
    pending: PublicLaneUnbonding,
    signed_genesis_hash: HashOf<iroha::data_model::block::BlockHeader>,
    network_id: NetworkId,
    retained_stake: Quantity,
}

impl WithdrawalLifecycle {
    fn unbond_intent(&self) -> PublicLanePreparationOperationV1 {
        PublicLanePreparationOperationV1::FinalizeUnbond(PublicLanePrepareUnbondV1 {
            validator: self.owner.account.clone(),
            staker: self.owner.account.clone(),
            request_id: self.pending.request_id,
        })
    }

    async fn prepare_until(
        &self,
        request: &PublicLanePreparationRequestV1,
        deadline: Instant,
    ) -> Result<PublicLanePreparationV1> {
        loop {
            let result = tokio::time::timeout_at(
                deadline.into(),
                self.owner
                    .client
                    .client()
                    .nexus()
                    .prepare_public_lane_plan(request),
            )
            .await
            .wrap_err("staking preparation exceeded its read deadline")?;
            match result {
                Ok(prepared) => return Ok(prepared),
                Err(error) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if let Some(delay) = committee_read_retry_delay(&error, remaining) {
                        sleep(delay).await;
                    } else {
                        return Err(error.into());
                    }
                }
            }
        }
    }

    async fn observe(&self, operation: PublicLanePreparationOperationV1) -> Result<Observation> {
        let request = PublicLanePreparationRequestV1 {
            lane_id: LaneId::SINGLE,
            valid_for_blocks: EPOCH,
            operation,
        };
        let deadline = Instant::now() + WAIT;
        let (prepared, supply) = loop {
            ensure!(
                Instant::now() < deadline,
                "staking observation deadline elapsed"
            );
            let prepared = self.prepare_until(&request, deadline).await?;
            let supply = tokio::time::timeout_at(
                deadline.into(),
                read_on_dedicated_thread({
                    let client = self.owner.client.clone();
                    let xor = self.escrow.definition().clone();
                    move || -> Result<Quantity> {
                        Ok(client
                            .client()
                            .with_request_deadline(deadline)
                            .query_single(FindAssetDefinitionById::new(xor))?
                            .total_quantity()
                            .clone())
                    }
                }),
            )
            .await
            .wrap_err("real XOR supply observation exceeded its original deadline")?
            .wrap_err("real XOR supply observation worker failed")?;
            // The definition query is a current-state read. Sandwich it between
            // two identical prepared views so its supply shares their exact tip.
            let confirmed = self.prepare_until(&request, deadline).await?;
            if !same_observed_tip(&prepared, &confirmed) {
                continue;
            }
            ensure!(
                prepared == confirmed,
                "one finalized staking tip returned inconsistent custody observations"
            );
            break (prepared, supply);
        };
        let (_, chain) = read_on_dedicated_thread({
            let client = self.owner.client.clone();
            let network_id = self.network_id;
            let genesis = self.signed_genesis_hash;
            let height = prepared.observed_height;
            move || read_contiguous_finality_chain(&client, network_id, genesis, height)
        })
        .await
        .wrap_err("staking observation finality worker failed")?;
        let tip = chain
            .last()
            .ok_or_else(|| eyre!("staking observation has no finality"))?;
        ensure!(
            prepared.network_id == self.network_id
                && prepared.request == request
                && prepared.xor_asset_definition_id == *self.escrow.definition()
                && prepared.observed_height == tip.height()
                && prepared.observed_block_hash == Hash::from(tip.block_hash())
                && prepared.observed_ledger_time_ms == tip.block_time_ms(),
            "staking observation does not identify the independently authenticated native tip"
        );
        for row in &prepared.balances {
            ensure!(
                row.balance >= row.stake_reserved.checked_add(&row.rewards_reserved)?,
                "real XOR balance does not cover additive stake and reward reserves"
            );
        }
        Ok(Observation {
            prepared,
            chain,
            supply,
        })
    }

    fn withdrawal(&self, observation: &Observation) -> Result<FinalizePublicLaneUnbond> {
        let PublicLanePreparedPlanV1::Monetary(plan) = &observation.prepared.plan else {
            return Err(eyre!("withdrawal preparation returned a reward claim"));
        };
        ensure!(
            plan.network_scope == PublicLaneMonetaryScopeV1::Network(self.network_id)
                && plan.source_asset == self.escrow
                && plan.destination_asset == self.destination
                && plan.amount == self.pending.amount
                && matches!(&plan.precondition, PublicLaneMonetaryPreconditionV1::Unbond(binding)
                    if binding.request_hash == public_lane_unbonding_commitment(&self.pending)?),
            "withdrawal lost its exact asset, quantity or retained slashing-liability commitment"
        );
        Ok(FinalizePublicLaneUnbond {
            lane_id: LaneId::SINGLE,
            validator: self.owner.account.clone(),
            staker: self.owner.account.clone(),
            request_id: self.pending.request_id,
            monetary_plan: plan.clone(),
        })
    }

    /// All processes were restarted while the owner was still a seven-seat voter.
    pub(super) async fn verify_retained_after_restart(&self) -> Result<()> {
        let observation = self.observe(self.unbond_intent()).await?;
        self.withdrawal(&observation)?;
        ensure!(
            observation.prepared.observed_height < TARGET_LAST
                && observation.asset(&self.escrow)?.stake_reserved == self.retained_stake
                && observation.chain.last().is_some_and(|tip| tip
                    .commitment()
                    .schedule
                    .current
                    .committee
                    .iter()
                    .any(|seat| seat.validator == self.owner.peer)),
            "restart lost the pending principal or released a still-voting validator"
        );
        Ok(())
    }

    async fn reject_early(&self, before: Observation, reason: &str) -> Result<Observation> {
        let transaction =
            submit_signed(&self.owner.client, self.withdrawal(&before)?.into(), false).await?;
        let after = self.observe(self.unbond_intent()).await?;
        self.withdrawal(&after)?;
        let fee = actual_fee(
            &before,
            &after,
            &transaction,
            false,
            Some(reason),
            self.escrow.definition(),
        )?;
        assert_effects(
            &before,
            &after,
            &self.escrow,
            &self.destination,
            &Quantity::zero(),
            &Quantity::zero(),
            &Quantity::zero(),
            &self.owner.account,
            &fee,
        )?;
        Ok(after)
    }

    /// Prove replacement, retain slashable custody, then withdraw the full real bond.
    pub(super) async fn complete_withdrawal(
        self,
        network: &sandbox::SerializedNetwork,
        voters: &[PeerId],
        expected: &BTreeSet<PeerId>,
    ) -> Result<()> {
        let after_replacement = self.observe(self.unbond_intent()).await?;
        ensure!(
            after_replacement.prepared.observed_height >= TARGET_LAST + 1
                && after_replacement.prepared.observed_height
                    < self.pending.liability_release_height,
            "replacement must precede the independently retained evidence horizon"
        );
        verify_equal_vote_context(
            after_replacement.chain.last().expect("authenticated tip"),
            expected,
        )?;
        self.reject_early(after_replacement, "remains slashable")
            .await?;
        advance_to_height(network, voters, self.pending.liability_release_height).await?;
        let deadline = Instant::now() + WAIT;
        let before = loop {
            let observation = self.observe(self.unbond_intent()).await?;
            if observation.prepared.observed_ledger_time_ms >= self.pending.release_at_ms {
                break observation;
            }
            ensure!(
                Instant::now() < deadline,
                "finite real-time unbond release deadline elapsed"
            );
            // Wait on the actual clock, then let a paid transaction establish ledger time.
            // Neither a fabricated timestamp nor empty-block production advances release.
            sleep(Duration::from_secs(1)).await;
            advance_to_height(network, voters, observation.prepared.observed_height + 1).await?;
        };
        let transaction =
            submit_signed(&self.owner.client, self.withdrawal(&before)?.into(), true).await?;
        // Preparing a possible future delegation is a read: it observes both exact assets
        // after the withdrawn validator and its zero stake position have been pruned.
        let survivor = voters.first().ok_or_else(|| eyre!("no remaining voter"))?;
        let survivor_account = network
            .validators()
            .iter()
            .chain(network.committee_validators())
            .find(|peer| peer.id() == *survivor)
            .ok_or_else(|| eyre!("remaining voter has no process"))?
            .account_id();
        let observation_intent = PublicLanePreparationOperationV1::Bond(PublicLanePrepareBondV1 {
            validator: survivor_account,
            staker: self.owner.account.clone(),
            amount: 1_u64.into(),
        });
        let after = self.observe(observation_intent.clone()).await?;
        let fee = actual_fee(
            &before,
            &after,
            &transaction,
            true,
            None,
            self.escrow.definition(),
        )?;
        assert_effects(
            &before,
            &after,
            &self.escrow,
            &self.destination,
            &self.pending.amount,
            &self.pending.amount,
            &Quantity::zero(),
            &self.owner.account,
            &fee,
        )?;
        let replay =
            submit_signed(&self.owner.client, self.withdrawal(&before)?.into(), false).await?;
        let after_replay = self.observe(observation_intent).await?;
        let replay_fee = actual_fee(
            &after,
            &after_replay,
            &replay,
            false,
            None,
            self.escrow.definition(),
        )?;
        assert_effects(
            &after,
            &after_replay,
            &self.escrow,
            &self.destination,
            &Quantity::zero(),
            &Quantity::zero(),
            &Quantity::zero(),
            &self.owner.account,
            &replay_fee,
        )?;
        for block in after_replay
            .chain
            .iter()
            .filter(|block| block.height() > TARGET_LAST)
        {
            verify_equal_vote_context(block, expected)?;
            ensure!(
                block.commitment().schedule.current.authority.generation == 2,
                "custody-release progress must retain the authenticated four-seat generation"
            );
        }
        Ok(())
    }
}

/// Quote and sign a fresh transaction; callers authenticate its exact execution output.
pub(super) async fn submit_signed(
    client: &Client,
    instruction: InstructionBox,
    applied: bool,
) -> Result<SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(AccountTransactionDraft::new(
        vec![instruction],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    ))?;
    let quote = account
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .await?;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "fee quote changed the selected monetary-plan signer"
    );
    payload.fee_payment = quote.intent;
    let transaction = account.sign_transaction(payload)?;
    let result = account.submit_transaction_and_wait(&transaction).await;
    if applied {
        result.wrap_err("signed monetary transaction failed")?;
    } else {
        ensure!(
            result.is_err(),
            "rejected monetary operation unexpectedly applied"
        );
    }
    // A submission error alone is not rejection evidence; the caller must find the
    // exact signed input and its rejected Network output in authenticated finality.
    Ok(transaction)
}

fn require_only_accounted_input(
    expected: Hash,
    mut inputs: impl Iterator<Item = Hash>,
) -> Result<()> {
    ensure!(
        inputs.next() == Some(expected) && inputs.next().is_none(),
        "accounting interval must contain exactly the signed monetary input and no unrelated transactions"
    );
    Ok(())
}

fn actual_fee(
    before: &Observation,
    after: &Observation,
    transaction: &SignedTransaction,
    applied: bool,
    rejection: Option<&str>,
    xor: &AssetDefinitionId,
) -> Result<Quantity> {
    let start = before.prepared.observed_height;
    let end = after.prepared.observed_height;
    ensure!(
        start < end
            && after.chain.iter().any(|block| block.height() == start
                && Hash::from(block.block_hash()) == before.prepared.observed_block_hash)
            && after.chain.last().is_some_and(|block| block.height() == end
                && Hash::from(block.block_hash()) == after.prepared.observed_block_hash),
        "accounting observations must bound one independently authenticated finality interval"
    );
    require_only_accounted_input(
        Hash::from(transaction.hash_as_entrypoint()),
        after
            .chain
            .iter()
            .filter(|block| block.height() > start)
            .flat_map(|block| block.block().network_entrypoints())
            .map(|input| Hash::from(input.hash())),
    )?;
    for block in after.chain.iter().filter(|block| block.height() > start) {
        for index in 0..block.block().network_entrypoint_count() {
            let Some(TransactionEntrypoint::External(input)) =
                block.block().network_entrypoint_at(index)
            else {
                continue;
            };
            if input.hash() != transaction.hash() {
                continue;
            }
            ensure!(
                input == transaction,
                "finalized monetary input differs from the signed transaction"
            );
            let (_, output) = block
                .block()
                .network_output_at(u32::try_from(index)?)
                .ok_or_else(|| eyre!("monetary input lost its explicit Network output"))?;
            ensure!(
                output.result.0.is_ok() == applied,
                "authenticated monetary outcome differs from expectation"
            );
            if let Some(reason) = rejection {
                ensure!(
                    output
                        .result
                        .0
                        .as_ref()
                        .err()
                        .is_some_and(|error| format!("{error:?}").contains(reason)),
                    "withdrawal rejected for a different reason than the retained obligation"
                );
            }
            let Some(receipt) = output.result.nexus_fee_receipt() else {
                ensure!(
                    !applied,
                    "applied monetary transaction omitted actual real-XOR fee settlement"
                );
                return Ok(Quantity::zero());
            };
            ensure!(
                receipt.source_id == *Hash::from(input.hash_as_entrypoint()).as_ref()
                    && receipt.block_height == block.height()
                    && receipt.fee_asset_id == *xor
                    && receipt.debit_source == FeeDebitSource::Account(input.authority().clone())
                    && matches!(receipt.settlement, NexusFeeSettlementV1::Burn)
                    && !receipt.fee_amount.is_zero(),
                "monetary execution must carry its actual separate authority-paid XOR burn"
            );
            return Ok(receipt.fee_amount.clone());
        }
    }
    Err(eyre!(
        "signed monetary transaction has no authenticated execution output"
    ))
}

#[allow(clippy::too_many_arguments)]
fn assert_effects(
    before: &Observation,
    after: &Observation,
    escrow: &AssetId,
    recipient: &AssetId,
    paid: &Quantity,
    released_stake: &Quantity,
    released_reward: &Quantity,
    fee_payer: &AccountId,
    fee: &Quantity,
) -> Result<()> {
    let source_before = before.asset(escrow)?;
    let source_after = after.asset(escrow)?;
    let recipient_before = before.asset(recipient)?;
    let recipient_after = after.asset(recipient)?;
    let source_fee = if escrow.account() == fee_payer {
        fee.clone()
    } else {
        Quantity::zero()
    };
    let recipient_fee = if recipient.account() == fee_payer {
        fee.clone()
    } else {
        Quantity::zero()
    };
    ensure!(
        source_before
            .balance
            .checked_sub(paid)?
            .checked_sub(&source_fee)?
            == source_after.balance
            && recipient_before
                .balance
                .checked_add(paid)?
                .checked_sub(&recipient_fee)?
                == recipient_after.balance
            && source_before.stake_reserved.checked_sub(released_stake)?
                == source_after.stake_reserved
            && source_before
                .rewards_reserved
                .checked_sub(released_reward)?
                == source_after.rewards_reserved
            && recipient_before.stake_reserved == recipient_after.stake_reserved
            && recipient_before.rewards_reserved == recipient_after.rewards_reserved
            && before.supply.checked_sub(fee)? == after.supply,
        "actual XOR movement, additive reserves or supply differ from principal/reward plus separate settled fees"
    );
    Ok(())
}

/// Reserve existing treasury XOR, claim its exact bounded record, and retain a full bond.
pub(super) async fn fund_rewards_and_schedule_withdrawal(
    network: &sandbox::SerializedNetwork,
    admin: &Client,
    owner: &Operator,
    genesis_validator: bool,
    return_preparation: &ValidatorCommitteePreparationV1,
    signed_genesis_hash: HashOf<iroha::data_model::block::BlockHeader>,
) -> Result<WithdrawalLifecycle> {
    let ingress = exact_process_roster(
        network,
        &[return_preparation.committee[0].validator.clone()],
    )?[0];
    let mut owner = owner.clone();
    owner.client = rebind_blocking_client(&owner.client, |builder| {
        builder.torii_url = ingress.client().client().endpoint().clone();
        builder.transaction_status_timeout = WAIT;
    });
    let admin = rebind_blocking_client(admin, |builder| {
        builder.torii_url = ingress.client().client().endpoint().clone();
    });
    let xor: AssetDefinitionId = TAIRA_XOR.parse()?;
    let escrow = validator_xor_escrow(&network.genesis(), &xor)?;
    ensure!(
        escrow.account() == &*ALICE_ID,
        "funded treasury must own the exact shared XOR custody"
    );
    let parameters = read_on_dedicated_thread({
        let admin = admin.clone();
        move || Ok(admin.client().query_single(FindParameters)?)
    })
    .await
    .wrap_err("signed staking policy worker failed")?;
    let npos = SumeragiNposParameters::from_custom_parameter(
        parameters
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .ok_or_else(|| eyre!("signed NPoS policy missing"))?,
    )?
    .ok_or_else(|| eyre!("signed NPoS policy invalid"))?;
    ensure!(
        npos.evidence_horizon_blocks == EPOCH * 2 && npos.slashing_delay_blocks == 1,
        "withdrawal must use the signed evidence and slashing horizons"
    );
    let pending = PublicLaneUnbonding {
        request_id: Hash::new_from_chunks(&[
            b"iroha:real-xor-withdrawal:v1",
            &norito::encode_canonical(&owner.account)?,
        ]),
        amount: if genesis_validator {
            1_000_u64.into()
        } else {
            2_000_u64.into()
        },
        release_at_ms: finite_release_deadline()?,
        slashable_through_height: return_preparation.last_height,
        liability_release_height: return_preparation
            .last_height
            .checked_add(npos.evidence_horizon_blocks)
            .and_then(|height| height.checked_add(npos.slashing_delay_blocks))
            .ok_or_else(|| eyre!("unbond liability horizon overflow"))?,
    };
    let mut lifecycle = WithdrawalLifecycle {
        destination: AssetId::with_scope(xor, owner.account.clone(), *escrow.scope()),
        owner,
        escrow,
        pending,
        signed_genesis_hash,
        network_id: network.network_id(),
        retained_stake: Quantity::zero(),
    };
    let before_schedule = lifecycle
        .observe(PublicLanePreparationOperationV1::Bond(
            PublicLanePrepareBondV1 {
                validator: lifecycle.owner.account.clone(),
                staker: lifecycle.owner.account.clone(),
                amount: 1_u64.into(),
            },
        ))
        .await?;
    lifecycle.pending.release_at_ms = finite_release_deadline()?;
    let scheduled = submit_signed(
        &lifecycle.owner.client,
        SchedulePublicLaneUnbond {
            lane_id: LaneId::SINGLE,
            validator: lifecycle.owner.account.clone(),
            staker: lifecycle.owner.account.clone(),
            request_id: lifecycle.pending.request_id,
            amount: lifecycle.pending.amount.clone(),
            release_at_ms: lifecycle.pending.release_at_ms,
        }
        .into(),
        true,
    )
    .await?;
    let before_reward = lifecycle.observe(lifecycle.unbond_intent()).await?;
    let schedule_fee = actual_fee(
        &before_schedule,
        &before_reward,
        &scheduled,
        true,
        None,
        lifecycle.escrow.definition(),
    )?;
    assert_effects(
        &before_schedule,
        &before_reward,
        &lifecycle.escrow,
        &lifecycle.destination,
        &Quantity::zero(),
        &Quantity::zero(),
        &Quantity::zero(),
        &lifecycle.owner.account,
        &schedule_fee,
    )?;
    lifecycle.withdrawal(&before_reward)?;
    lifecycle.retained_stake = before_reward
        .asset(&lifecycle.escrow)?
        .stake_reserved
        .clone();
    ensure!(
        before_reward.prepared.observed_height < TARGET_LAST,
        "unbond scheduling missed the seven-seat tenure"
    );
    let reward: Quantity = 25_u64.into();
    let record = submit_signed(
        &admin,
        RecordPublicLaneRewards {
            lane_id: LaneId::SINGLE,
            epoch: REWARD_EPOCH,
            reward_asset: lifecycle.escrow.clone(),
            total_reward: reward.clone(),
            shares: vec![PublicLaneRewardShare {
                account: lifecycle.owner.account.clone(),
                role: PublicLaneRewardRole::Validator,
                amount: reward.clone(),
            }],
            metadata: Metadata::default(),
        }
        .into(),
        true,
    )
    .await?;
    let claim = lifecycle
        .observe(PublicLanePreparationOperationV1::ClaimRewards(
            PublicLanePrepareClaimV1 {
                recipient: lifecycle.owner.account.clone(),
                upto_epoch: Some(REWARD_EPOCH),
                max_records: 1,
                accrued_sources: Vec::new(),
            },
        ))
        .await?;
    let fee = actual_fee(
        &before_reward,
        &claim,
        &record,
        true,
        None,
        lifecycle.escrow.definition(),
    )?;
    let before = before_reward.asset(&lifecycle.escrow)?;
    let reserved = claim.asset(&lifecycle.escrow)?;
    let recipient_before = before_reward.asset(&lifecycle.destination)?;
    let recipient_after = claim.asset(&lifecycle.destination)?;
    ensure!(
        before.balance.checked_sub(&fee)? == reserved.balance
            && before.stake_reserved == reserved.stake_reserved
            && before.rewards_reserved.checked_add(&reward)? == reserved.rewards_reserved
            && recipient_before == recipient_after
            && before_reward.supply.checked_sub(&fee)? == claim.supply,
        "reward distribution must reserve funded treasury XOR without minting or consuming stake"
    );
    let PublicLanePreparedPlanV1::Claim(plan) = &claim.prepared.plan else {
        return Err(eyre!("reward read returned withdrawal"));
    };
    ensure!(
        plan.records.len() == 1
            && plan.records[0].epoch == REWARD_EPOCH
            && plan.sources.len() == 1
            && plan.sources[0].source_asset == lifecycle.escrow
            && plan.sources[0].destination_asset == lifecycle.destination
            && plan.sources[0].payout == reward
            && plan.fee_claim.is_none(),
        "bounded reward plan differs from funded treasury distribution"
    );
    let transaction = submit_signed(
        &lifecycle.owner.client,
        ClaimPublicLaneRewards {
            lane_id: LaneId::SINGLE,
            account: lifecycle.owner.account.clone(),
            claim_plan: plan.clone(),
        }
        .into(),
        true,
    )
    .await?;
    let after_claim = lifecycle.observe(lifecycle.unbond_intent()).await?;
    let fee = actual_fee(
        &claim,
        &after_claim,
        &transaction,
        true,
        None,
        lifecycle.escrow.definition(),
    )?;
    assert_effects(
        &claim,
        &after_claim,
        &lifecycle.escrow,
        &lifecycle.destination,
        &reward,
        &Quantity::zero(),
        &reward,
        &lifecycle.owner.account,
        &fee,
    )?;
    let replay = submit_signed(
        &lifecycle.owner.client,
        ClaimPublicLaneRewards {
            lane_id: LaneId::SINGLE,
            account: lifecycle.owner.account.clone(),
            claim_plan: plan.clone(),
        }
        .into(),
        false,
    )
    .await?;
    ensure!(
        replay.hash() != transaction.hash(),
        "claim replay must be a new signed transaction carrying the exact consumed plan"
    );
    let after_replay = lifecycle.observe(lifecycle.unbond_intent()).await?;
    let replay_fee = actual_fee(
        &after_claim,
        &after_replay,
        &replay,
        false,
        Some("reward claim processing cursor changed after signing"),
        lifecycle.escrow.definition(),
    )?;
    assert_effects(
        &after_claim,
        &after_replay,
        &lifecycle.escrow,
        &lifecycle.destination,
        &Quantity::zero(),
        &Quantity::zero(),
        &Quantity::zero(),
        &lifecycle.owner.account,
        &replay_fee,
    )?;
    lifecycle
        .reject_early(
            after_replay,
            "authenticated release of current and frozen committee obligations",
        )
        .await?;
    Ok(lifecycle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_test_samples::BOB_ID;

    fn observation(
        source: u64,
        recipient: u64,
        stake: u64,
        reward: u64,
        supply: u64,
    ) -> Observation {
        let xor: AssetDefinitionId = TAIRA_XOR.parse().expect("canonical real XOR");
        let escrow = AssetId::new(xor.clone(), ALICE_ID.clone());
        let destination = AssetId::new(xor.clone(), BOB_ID.clone());
        let request = PublicLanePreparationRequestV1 {
            lane_id: LaneId::SINGLE,
            valid_for_blocks: EPOCH,
            operation: PublicLanePreparationOperationV1::ClaimRewards(PublicLanePrepareClaimV1 {
                recipient: BOB_ID.clone(),
                upto_epoch: Some(REWARD_EPOCH),
                max_records: 1,
                accrued_sources: Vec::new(),
            }),
        };
        let hash = Hash::new(b"accounting helper observation; not network qualification");
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(hash));
        Observation {
            prepared: PublicLanePreparationV1 {
                request,
                network_id,
                observed_height: 1,
                observed_block_hash: hash,
                observed_ledger_time_ms: 1,
                assumed_execution_height: 2,
                xor_asset_definition_id: xor,
                plan: PublicLanePreparedPlanV1::Monetary(PublicLaneMonetaryPlanV1 {
                    network_scope: PublicLaneMonetaryScopeV1::Network(network_id),
                    valid_until_height: EPOCH,
                    source_asset: escrow.clone(),
                    destination_asset: destination.clone(),
                    amount: 1_u64.into(),
                    precondition: PublicLaneMonetaryPreconditionV1::Unbond(
                        iroha::data_model::nexus::PublicLaneMonetaryUnbondV1 {
                            activation_height: 1,
                            request_hash: hash,
                        },
                    ),
                }),
                balances: vec![
                    PublicLanePreparationBalanceV1 {
                        asset: escrow,
                        balance: source.into(),
                        stake_reserved: stake.into(),
                        rewards_reserved: reward.into(),
                    },
                    PublicLanePreparationBalanceV1 {
                        asset: destination,
                        balance: recipient.into(),
                        stake_reserved: Quantity::zero(),
                        rewards_reserved: Quantity::zero(),
                    },
                ],
            },
            chain: Vec::new(),
            supply: supply.into(),
        }
    }

    #[test]
    fn supply_observation_retries_a_changed_height_or_finalized_hash() {
        let observed = observation(1_000, 100, 800, 25, 1_100).prepared;
        assert!(same_observed_tip(&observed, &observed));
        let mut advanced = observed.clone();
        advanced.observed_height += 1;
        assert!(!same_observed_tip(&observed, &advanced));
        let mut replaced = observed.clone();
        replaced.observed_block_hash = Hash::new(b"different finalized tip");
        assert!(!same_observed_tip(&observed, &replaced));
    }

    #[test]
    fn accounting_interval_rejects_missing_repeated_and_unrelated_paid_inputs() -> Result<()> {
        let expected = Hash::new(b"exact signed withdrawal");
        let progress = Hash::new(b"unrelated paid progress input");
        require_only_accounted_input(expected, [expected].into_iter())?;
        for inputs in [
            vec![],
            vec![progress],
            vec![expected, expected],
            vec![progress, expected],
            vec![expected, progress],
        ] {
            assert!(require_only_accounted_input(expected, inputs.into_iter()).is_err());
        }
        Ok(())
    }

    #[test]
    fn real_xor_accounting_separates_fees_principal_and_reward_reserves() -> Result<()> {
        let before = observation(1_000, 100, 800, 25, 1_100);
        let after_reward = observation(975, 123, 800, 0, 1_098);
        let escrow = &before.prepared.balances[0].asset;
        let recipient = &before.prepared.balances[1].asset;
        let after_schedule = observation(1_000, 98, 800, 25, 1_098);
        assert_effects(
            &before,
            &after_schedule,
            escrow,
            recipient,
            &Quantity::zero(),
            &Quantity::zero(),
            &Quantity::zero(),
            &BOB_ID,
            &2_u64.into(),
        )?;
        assert_effects(
            &before,
            &after_reward,
            escrow,
            recipient,
            &25_u64.into(),
            &Quantity::zero(),
            &25_u64.into(),
            &BOB_ID,
            &2_u64.into(),
        )?;
        let after_withdrawal = observation(800, 298, 600, 25, 1_098);
        assert_effects(
            &before,
            &after_withdrawal,
            escrow,
            recipient,
            &200_u64.into(),
            &200_u64.into(),
            &Quantity::zero(),
            &BOB_ID,
            &2_u64.into(),
        )?;
        for incorrect in [
            observation(975, 125, 800, 0, 1_098), // fee disguised as free principal
            observation(975, 123, 775, 0, 1_098), // reward consumes bonded custody
            observation(975, 123, 800, 0, 1_100), // fee receipt without real supply burn
        ] {
            assert!(
                assert_effects(
                    &before,
                    &incorrect,
                    escrow,
                    recipient,
                    &25_u64.into(),
                    &Quantity::zero(),
                    &25_u64.into(),
                    &BOB_ID,
                    &2_u64.into()
                )
                .is_err()
            );
        }
        for reserve in [true, false] {
            let mut changed_recipient = observation(975, 123, 800, 0, 1_098);
            if reserve {
                changed_recipient.prepared.balances[1].stake_reserved = 1_u64.into();
            } else {
                changed_recipient.prepared.balances[1].rewards_reserved = 1_u64.into();
            }
            assert!(
                assert_effects(
                    &before,
                    &changed_recipient,
                    escrow,
                    recipient,
                    &25_u64.into(),
                    &Quantity::zero(),
                    &25_u64.into(),
                    &BOB_ID,
                    &2_u64.into(),
                )
                .is_err(),
                "reward payment cannot create unrelated recipient reservations"
            );
        }
        Ok(())
    }

    #[test]
    fn rejected_withdrawal_accounting_preserves_all_custody() -> Result<()> {
        let before = observation(1_000, 100, 800, 25, 1_100);
        let unchanged = observation(1_000, 100, 800, 25, 1_100);
        let escrow = &before.prepared.balances[0].asset;
        let recipient = &before.prepared.balances[1].asset;
        assert_effects(
            &before,
            &unchanged,
            escrow,
            recipient,
            &Quantity::zero(),
            &Quantity::zero(),
            &Quantity::zero(),
            &BOB_ID,
            &Quantity::zero(),
        )?;
        let lost_pending_principal = observation(1_000, 100, 600, 25, 1_100);
        assert!(
            assert_effects(
                &before,
                &lost_pending_principal,
                escrow,
                recipient,
                &Quantity::zero(),
                &Quantity::zero(),
                &Quantity::zero(),
                &BOB_ID,
                &Quantity::zero()
            )
            .is_err()
        );
        Ok(())
    }
}
