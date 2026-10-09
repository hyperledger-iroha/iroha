//! A genuine Parliament pulse while the next committee's credentials remain pending.

use super::*;
use iroha::data_model::{
    consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconChainAnchorV1,
        GlobalThresholdBeaconKeySessionV1, GlobalThresholdBeaconPulseContextV1,
    },
    governance::types::{
        AbiVersion, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId, DeployContractProposal,
        GovernanceAttemptId, ParliamentBody, ProposalKind,
    },
    isi::governance::{
        CreateParliamentGovernanceAttemptV1, ParliamentConsumeSortitionPulseBatchV1,
        ParliamentLifecycleTransitionV1, ProposeDeployContract, RegisterCitizen,
        SubmitParliamentLifecycleTransitionV1,
    },
    permission::Permission,
    smart_contract::ContractAddress,
    sumeragi::epoch::BeaconEpochBindingV1,
};
use iroha_core::{
    beacon::{
        global_threshold_beacon_governance_seed_v1, global_threshold_beacon_npos_successor_seed_v1,
        verify_finalized_global_threshold_beacon_pulse_v1,
    },
    governance::parliament::ParliamentAttemptStateV1,
};
use iroha_executor_data_model::permission::{
    governance::CanProposeContractDeployment, smart_contract::CanManageSmartContractCode,
};

const ADDRESS: &str = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
const CITIZENS: usize = 24;
const BODY_SEATS: i64 = 3;
// Four actual transfer legs plus their fee remain below the canonical source bounds.
const CITIZEN_FUNDING_BATCH: usize = 4;
const CITIZEN_FEE_ALLOCATION: u64 = 10_000;

fn citizen_key(index: usize) -> Result<KeyPair> {
    KeyPair::try_from_seed(
        format!("committee-parliament-citizen-{index}").into_bytes(),
        iroha::crypto::Algorithm::Ed25519,
    )
    .map_err(Into::into)
}

fn citizenship_escrow() -> Result<AccountId> {
    Ok(AccountId::new(
        KeyPair::try_from_seed(
            b"committee-parliament-citizenship-escrow".to_vec(),
            iroha::crypto::Algorithm::Ed25519,
        )?
        .public_key()
        .clone(),
    ))
}

/// Seed identities and permissions; real XOR funding waits for its signed genesis definition.
pub(super) fn genesis(mut builder: NetworkBuilder) -> Result<NetworkBuilder> {
    let xor: AssetDefinitionId = defaults::nexus::staking::stake_asset_id().parse()?;
    let escrow = citizenship_escrow()?;
    builder = builder
        .with_genesis_instruction(Register::account(Account::new(escrow.clone())))
        .with_config_layer(|layer| {
            layer
                .write(["gov", "citizenship_asset_id"], xor.to_string())
                .write(["gov", "citizenship_escrow_account"], escrow.to_string())
                .write(["gov", "parliament_alternate_size"], 0_i64)
                .write(["gov", "rules_committee_size"], BODY_SEATS)
                .write(["gov", "agenda_council_size"], BODY_SEATS)
                .write(["gov", "interest_panel_size"], BODY_SEATS)
                .write(["gov", "review_panel_size"], BODY_SEATS)
                .write(["gov", "oversight_committee_size"], BODY_SEATS)
                .write(["gov", "policy_jury_size"], BODY_SEATS);
        })
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageSmartContractCode),
            ALICE_ID.clone(),
        ))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanProposeContractDeployment {
                contract_address: ADDRESS.parse()?,
            }),
            ALICE_ID.clone(),
        ));
    for index in 0..CITIZENS {
        let citizen = AccountId::new(citizen_key(index)?.public_key().clone());
        builder = builder.with_genesis_instruction(Register::account(Account::new(citizen)));
    }
    Ok(builder)
}

/// Fund exact account-owned XOR after bootstrap, then bind each citizen's own signed bond.
pub(super) async fn fund_and_register_citizens(
    network: &sandbox::SerializedNetwork,
    admin: &Client,
    xor: &AssetDefinitionId,
    signed_genesis_hash: iroha::crypto::HashOf<iroha::data_model::block::BlockHeader>,
) -> Result<()> {
    ensure!(
        admin.client().account() == &*ALICE_ID,
        "citizens use the exact funded bootstrap owner"
    );
    let bond = defaults::governance::citizenship_bond_amount();
    ensure!(
        !bond.is_zero(),
        "citizenship retains its actual positive XOR bond"
    );
    let funding = bond.try_add(&Quantity::from(CITIZEN_FEE_ALLOCATION))?;
    let citizens = (0..CITIZENS)
        .map(|index| {
            let key = citizen_key(index)?;
            Ok((AccountId::new(key.public_key().clone()), key))
        })
        .collect::<Result<Vec<_>>>()?;
    let (before_height, before_owner, before_supply) = read_on_dedicated_thread({
        let admin = admin.clone();
        let xor = xor.clone();
        move || -> Result<_> {
            let deadline = Instant::now() + WAIT;
            let height = committee_status::height_until_blocking(&admin, deadline)?;
            let owner = admin
                .client()
                .query_single(FindAssetById::new(AssetId::new(
                    xor.clone(),
                    ALICE_ID.clone(),
                )))?;
            let definition = admin
                .client()
                .query_single(FindAssetDefinitionById::new(xor))?;
            ensure!(
                committee_status::height_until_blocking(&admin, deadline)? == height,
                "citizen funding prestate changed during observation"
            );
            Ok((
                height,
                owner.value().clone(),
                definition.total_quantity().clone(),
            ))
        }
    })
    .await?;
    let setup_blocks = CITIZENS.div_ceil(CITIZEN_FUNDING_BATCH) + CITIZENS;
    ensure!(
        before_height >= 5 && before_height + u64::try_from(setup_blocks)? + 4 < SELECTION,
        "positive citizen funding and canonical proposal must finish before frozen selection"
    );
    let mut funding_transactions = Vec::new();
    for group in citizens.chunks(CITIZEN_FUNDING_BATCH) {
        let transfers = group.iter().map(|(citizen, _)| {
            Transfer::asset_quantity(
                AssetId::new(xor.clone(), ALICE_ID.clone()),
                funding.clone(),
                citizen.clone(),
            )
        });
        let transaction =
            parliament_submission::prepare_parliament_transaction(admin, transfers).await?;
        let applied = admin
            .account_client()
            .submit_transaction_and_wait(&transaction)
            .await?;
        ensure!(
            applied == transaction.hash(),
            "citizen funding changed its exact signed source"
        );
        funding_transactions.push(transaction);
    }
    let mut registrations = Vec::new();
    for (citizen, key) in &citizens {
        let client = rebind_blocking_client(
            &network.validators()[0].client_for(citizen, key.private_key().clone()),
            |builder| {
                builder.transaction_status_timeout = WAIT;
            },
        );
        registrations.push(
            committee_staking::submit_signed(
                &client,
                RegisterCitizen {
                    owner: citizen.clone(),
                    amount: bond.clone(),
                }
                .into(),
                true,
            )
            .await?,
        );
    }
    let (height, owner_balance, supply, escrow_balance, balances) = read_on_dedicated_thread({
        let admin = admin.clone();
        let xor = xor.clone();
        let escrow = citizenship_escrow()?;
        let citizens = citizens
            .iter()
            .map(|(account, _)| account.clone())
            .collect::<Vec<_>>();
        move || -> Result<_> {
            let deadline = Instant::now() + WAIT;
            let height = committee_status::height_until_blocking(&admin, deadline)?;
            let balance = |account| -> Result<Quantity> {
                Ok(admin
                    .client()
                    .query_single(FindAssetById::new(AssetId::new(xor.clone(), account)))?
                    .value()
                    .clone())
            };
            let owner = balance(ALICE_ID.clone())?;
            let escrow = balance(escrow)?;
            let balances = citizens
                .into_iter()
                .map(balance)
                .collect::<Result<Vec<_>>>()?;
            let supply = admin
                .client()
                .query_single(FindAssetDefinitionById::new(xor))?
                .total_quantity()
                .clone();
            ensure!(
                committee_status::height_until_blocking(&admin, deadline)? == height,
                "citizen funding balances moved during observation"
            );
            Ok((height, owner, supply, escrow, balances))
        }
    })
    .await?;
    ensure!(
        height + 4 < SELECTION,
        "paid citizenship setup reached the selecting boundary"
    );
    let (_, chain) = read_on_dedicated_thread({
        let admin = admin.clone();
        let network_id = network.network_id();
        move || read_contiguous_finality_chain(&admin, network_id, signed_genesis_hash, height)
    })
    .await?;
    let mut funding_fees = Quantity::zero();
    for transaction in &funding_transactions {
        funding_fees =
            funding_fees.try_add(&require_paid_and_applied(&chain, transaction, xor)?)?;
    }
    let mut total_fees = funding_fees.clone();
    let mut total_funding = Quantity::zero();
    let mut total_bond = Quantity::zero();
    for (transaction, balance) in registrations.iter().zip(balances) {
        let fee = require_paid_and_applied(&chain, transaction, xor)?;
        ensure!(
            balance == funding.try_sub(&bond)?.try_sub(&fee)?
                && balance >= fee
                && !balance.is_zero(),
            "citizen's exact XOR bond and separate actual fee must leave funded spendable balance"
        );
        total_fees = total_fees.try_add(&fee)?;
        total_funding = total_funding.try_add(&funding)?;
        total_bond = total_bond.try_add(&bond)?;
    }
    ensure!(
        escrow_balance == total_bond
            && owner_balance
                == before_owner
                    .try_sub(&total_funding)?
                    .try_sub(&funding_fees)?
            && supply == before_supply.try_sub(&total_fees)?,
        "paid citizen setup must conserve actual XOR bonds and burn only its finalized fees"
    );
    Ok(())
}

/// Admit a real canonical contract proposal before the committee selection cutoff.
pub(super) async fn stage_proposal(admin: &Client) -> Result<ProposalKind> {
    let artifact = parliament_submission::minimal_contract_artifact_with_identity(
        "CommitteePreparationPulse",
        "integration-tests",
    );
    let (code_hash, abi_hash) =
        parliament_submission::stage_contract_artifact(admin, &artifact).await?;
    let contract_address: ContractAddress = ADDRESS.parse()?;
    let proposal = ProposalKind::DeployContract(DeployContractProposal {
        proposal_operator: admin.client().account().clone(),
        contract_address: contract_address.clone(),
        code_hash,
        abi_hash,
        abi_version: AbiVersion::new(1),
        manifest_provenance: None,
    });
    committee_staking::submit_signed(
        admin,
        ProposeDeployContract {
            contract_address,
            code_hash,
            abi_hash,
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        }
        .into(),
        true,
    )
    .await?;
    let height = read_on_dedicated_thread({
        let admin = admin.clone();
        move || committee_status::height_until_blocking(&admin, Instant::now() + WAIT)
    })
    .await?;
    ensure!(
        height < SELECTION,
        "admitted Parliament proposal missed the unfrozen setup interval"
    );
    Ok(proposal)
}

async fn attempt(admin: &Client, id: GovernanceAttemptId) -> Result<ParliamentAttemptStateV1> {
    let response = read_on_dedicated_thread({
        let client = admin.client().clone();
        move || client.get_parliament_attempt(id)
    })
    .await?;
    norito::decode_canonical(&hex::decode(response.state_payload_hex)?)
        .wrap_err("decode the actual Parliament reducer projection")
}

fn verified_pulse(
    chain: &[CertifiedBlock],
    height: u64,
    session: &GlobalThresholdBeaconKeySessionV1,
) -> Result<FinalizedGlobalThresholdBeaconPulseV1> {
    let block = chain
        .iter()
        .find(|block| block.height() == height)
        .ok_or_else(|| eyre!("requested Parliament pulse has no independently certified block"))?;
    let parent = chain
        .iter()
        .find(|block| block.height() + 1 == height)
        .ok_or_else(|| eyre!("requested pulse has no independently certified parent"))?;
    let pulse = block
        .commitment()
        .beacon
        .ok_or_else(|| eyre!("requested height lacks the mandatory signed pulse"))?;
    let header = block
        .header()
        .ok_or_else(|| eyre!("pulse cannot use a genesis-only context"))?;
    let context = GlobalThresholdBeaconPulseContextV1 {
        instance: header.instance.0,
        epoch: header.epoch.epoch,
        epoch_context_id: header.epoch.context.0,
        parent_consensus_hash: header.parent_hash.0,
        parent_result: header.parent_result.0,
    };
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: session.network_id,
        session_id: session.session_id,
        roster_hash: session.roster_hash,
        transcript_hash: session.transcript_hash,
    };
    let validated = validate_global_threshold_beacon_session_v1(
        session,
        &binding,
        &iroha_allocation::AllocationBudget::new(64 * 1024 * 1024),
    )?;
    verify_finalized_global_threshold_beacon_pulse_v1(
        &validated,
        &pulse,
        GlobalThresholdBeaconChainAnchorV1 {
            height: parent.height(),
            block_hash: parent.block_hash(),
        },
        &context,
    )?;
    ensure!(
        matches!(block.commitment().schedule.current.authorization.beacon,
            BeaconEpochBindingV1::Installed(installed)
                if installed.session_id == session.session_id && installed.transcript_hash == session.transcript_hash),
        "pulse signer must be the installed current session from independently authenticated finality"
    );
    Ok(pulse)
}

/// Request, finalize and consume genuine sortition while both credential sets are installed.
#[allow(clippy::too_many_arguments)]
pub(super) async fn exercise(
    network: &sandbox::SerializedNetwork,
    admin: &Client,
    voters: &BTreeSet<PeerId>,
    preparation: &ValidatorCommitteePreparationV1,
    current_session: &GlobalThresholdBeaconKeySessionV1,
    pending_session: &GlobalThresholdBeaconKeySessionV1,
    signed_genesis_hash: iroha::crypto::HashOf<iroha::data_model::block::BlockHeader>,
    proposal: ProposalKind,
) -> Result<FinalizedGlobalThresholdBeaconPulseV1> {
    let before = read_validator_committee(admin, preparation.target_epoch).await?;
    ensure!(
        before.pending_beacon_session.as_ref() == Some(pending_session)
            && current_session.session_id != pending_session.session_id
            && before
                .selected
                .as_ref()
                .is_some_and(|row| row.transition.readiness.len() == preparation.committee.len()),
        "Parliament pulse must run with complete pending custody alongside current credentials"
    );
    let create = CreateParliamentGovernanceAttemptV1 {
        proposal,
        attempt_sequence: 0,
    };
    let id = create.governance_attempt_id();
    let instructions: Vec<InstructionBox> = vec![
        create.into(),
        SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: id,
            transition: ParliamentLifecycleTransitionV1::CompleteQualification,
        }
        .into(),
        SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: id,
            transition: ParliamentLifecycleTransitionV1::RegisterInitialSortition,
        }
        .into(),
    ];
    let request_transaction =
        parliament_submission::prepare_parliament_transaction(admin, instructions).await?;
    let applied = admin
        .account_client()
        .submit_transaction_and_wait(&request_transaction)
        .await?;
    ensure!(
        applied == request_transaction.hash(),
        "Parliament request acknowledgement changed its signed source"
    );
    let registered = attempt(admin, id).await?;
    let elections = registered
        .required_bodies()
        .iter()
        .filter(|required| required.body != ParliamentBody::ConfirmationJury)
        .map(|required| BodyElectionAttemptId::derive_v1(id, required.body, 0))
        .collect::<Vec<_>>();
    ensure!(
        elections.len() == 6,
        "real contract proposal must require its complete six-body draw"
    );
    let requests = elections
        .iter()
        .map(|election| {
            registered
                .election(election)
                .map(|election| election.attempt().request)
                .ok_or_else(|| eyre!("canonical initial sortition omitted a required body"))
        })
        .collect::<Result<Vec<_>>>()?;
    let pulse_height = requests[0].pulse_height;
    ensure!(
        requests
            .iter()
            .all(|request| request.pulse_height == pulse_height
                && request.candidate_count == CITIZENS as u32
                && request.target_seats == BODY_SEATS as u32
                && request.request_height + registered.sortition_pulse_delay_blocks()
                    == pulse_height)
            && pulse_height > preparation.selection_height
            && pulse_height + 1 < CUTOFF - 1,
        "queued Parliament pulse must occur inside E+1 and before the separate epoch-boundary pulse"
    );
    let roster = voters.iter().cloned().collect::<Vec<_>>();
    advance_to_height(network, &roster, pulse_height).await?;
    let (_, chain) = read_on_dedicated_thread({
        let admin = admin.clone();
        let network_id = network.network_id();
        move || {
            read_contiguous_finality_chain(&admin, network_id, signed_genesis_hash, pulse_height)
        }
    })
    .await?;
    let pulse = verified_pulse(&chain, pulse_height, current_session)?;
    require_paid_and_applied(
        &chain,
        &request_transaction,
        &preparation.eligibility.xor_asset_definition_id,
    )?;
    for block in chain
        .iter()
        .filter(|block| block.height() > preparation.selection_height)
    {
        verify_equal_vote_context(block, voters)?;
        ensure!(
            block
                .commitment()
                .schedule
                .current
                .authorization
                .authority_generation
                == 0
                && block.commitment().schedule.current.authorization.epoch == 1,
            "queued Parliament pulse cannot activate the pending generation early"
        );
    }
    let mut request_ids = requests
        .iter()
        .map(|request| request.id)
        .collect::<Vec<_>>();
    request_ids.sort_unstable();
    let consumer = committee_staking::submit_signed(
        admin,
        SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: id,
            transition: ParliamentLifecycleTransitionV1::ConsumeSortitionPulseBatch(
                ParliamentConsumeSortitionPulseBatchV1 {
                    request_ids,
                    beacon_session_id: BeaconSessionId::for_network_v1(&network.network_id()),
                    pulse_height,
                    pulse_id: BeaconPulseId::new(pulse.pulse_id),
                },
            ),
        }
        .into(),
        true,
    )
    .await?;
    let (_, consumed_chain) = read_on_dedicated_thread({
        let admin = admin.clone();
        let network_id = network.network_id();
        move || {
            read_contiguous_finality_chain(
                &admin,
                network_id,
                signed_genesis_hash,
                pulse_height + 1,
            )
        }
    })
    .await?;
    require_paid_and_applied(
        &consumed_chain,
        &consumer,
        &preparation.eligibility.xor_asset_definition_id,
    )?;
    let consumed_tip = consumed_chain
        .last()
        .ok_or_else(|| eyre!("pulse consumer lacks finality"))?;
    verify_equal_vote_context(consumed_tip, voters)?;
    ensure!(
        consumed_tip.commitment().schedule.current
            == chain
                .last()
                .expect("verified pulse")
                .commitment()
                .schedule
                .current,
        "consuming Parliament entropy cannot change current signing or scheduling authority"
    );
    let consumed = attempt(admin, id).await?;
    let governance_seed = global_threshold_beacon_governance_seed_v1(&pulse, pulse_height);
    ensure!(
        governance_seed != pulse.seed
            && elections
                .iter()
                .all(|id| consumed
                    .election(id)
                    .is_some_and(|election| election.pulse_id()
                        == Some(BeaconPulseId::new(pulse.pulse_id))
                        && election.pulse_output() == Some(governance_seed)
                        && election.primary_assignments().len() == BODY_SEATS as usize)),
        "every genuine Parliament body must consume the exact domain-separated pulse"
    );
    let after = read_validator_committee(admin, preparation.target_epoch).await?;
    ensure!(
        after.pending_beacon_session.as_ref() == Some(pending_session)
            && after
                .selected
                .as_ref()
                .is_some_and(|row| row.transition.preparation == *preparation
                    && row.transition.outcome.is_none()),
        "consuming governance entropy cannot publish the pending committee or beacon session"
    );
    Ok(pulse)
}

fn require_paid_and_applied(
    chain: &[CertifiedBlock],
    transaction: &SignedTransaction,
    xor: &AssetDefinitionId,
) -> Result<Quantity> {
    for block in chain {
        for index in 0..block.block().network_entrypoint_count() {
            if matches!(block.block().network_entrypoint_at(index),
                Some(TransactionEntrypoint::External(input)) if input == transaction)
            {
                let (_, output) = block
                    .block()
                    .network_output_at(u32::try_from(index)?)
                    .ok_or_else(|| {
                        eyre!("Parliament signed input has no explicit execution output")
                    })?;
                ensure!(
                    output.result.0.is_ok(),
                    "Parliament signed operation did not apply in authenticated finality"
                );
                let receipt = output
                    .result
                    .nexus_fee_receipt()
                    .ok_or_else(|| eyre!("Parliament signed operation lacks its actual XOR fee"))?;
                ensure!(
                    receipt.source_id == *Hash::from(transaction.hash_as_entrypoint()).as_ref()
                        && receipt.block_height == block.height()
                        && receipt.fee_asset_id == *xor
                        && receipt.debit_source
                            == iroha::data_model::nexus::FeeDebitSource::Account(
                                transaction.authority().clone()
                            )
                        && matches!(
                            receipt.settlement,
                            iroha::data_model::block::consensus::NexusFeeSettlementV1::Burn
                        )
                        && !receipt.fee_amount.is_zero(),
                    "Parliament fee must bind the exact signed source and account-owned XOR burn"
                );
                return Ok(receipt.fee_amount.clone());
            }
        }
    }
    Err(eyre!(
        "Parliament signed operation is absent from the independently authenticated prefix"
    ))
}

/// The later fresh pre-boundary pulse alone selects the next scheduling seed.
pub(super) fn verify_boundary(
    chain: &[CertifiedBlock],
    governance_pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    current_session: &GlobalThresholdBeaconKeySessionV1,
) -> Result<()> {
    let pulse = verified_pulse(chain, CUTOFF - 1, current_session)?;
    let cutoff = chain
        .last()
        .ok_or_else(|| eyre!("missing actual epoch cutoff"))?;
    let boundary = cutoff
        .commitment()
        .schedule
        .boundary
        .as_ref()
        .ok_or_else(|| eyre!("epoch cutoff omitted its mandatory decision"))?;
    for block in chain.iter().filter(|block| block.height() > SELECTION) {
        ensure!(
            block
                .commitment()
                .schedule
                .current
                .authorization
                .authority_generation
                == 0
                && block.commitment().schedule.current.authorization.epoch == 1
                && matches!(block.commitment().schedule.current.authorization.beacon,
                BeaconEpochBindingV1::Installed(installed)
                    if installed.session_id == current_session.session_id && installed.transcript_hash == current_session.transcript_hash),
            "pending committee and beacon custody must remain inactive through the exact cutoff"
        );
    }
    ensure!(
        cutoff.height() == CUTOFF
            && governance_pulse.height < pulse.height
            && governance_pulse.pulse_id != pulse.pulse_id
            && boundary.next.leader_seed
                == global_threshold_beacon_npos_successor_seed_v1(&pulse, CUTOFF, 2)
            && boundary.next.leader_seed
                != global_threshold_beacon_npos_successor_seed_v1(governance_pulse, CUTOFF, 2)
            && boundary.next.leader_seed
                != global_threshold_beacon_governance_seed_v1(
                    governance_pulse,
                    governance_pulse.height
                )
            && cutoff
                .commitment()
                .schedule
                .current
                .authorization
                .authority_generation
                == 0
            && boundary.next.authorization.authority_generation == 1,
        "only the exact fresh boundary pulse may schedule activation of the prepared generation"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_defines_real_xor_before_any_funding_and_leaves_citizen_bonds_for_signed_setup()
    -> Result<()> {
        init_instruction_registry();
        let network = genesis(
            NetworkBuilder::new()
                .with_peers(4)
                .with_npos_genesis_bootstrap(1_000_u64.into()),
        )?
        .build();
        let xor: AssetDefinitionId = defaults::nexus::staking::stake_asset_id().parse()?;
        let citizens = (0..CITIZENS)
            .map(|index| Ok(AccountId::new(citizen_key(index)?.public_key().clone())))
            .collect::<Result<BTreeSet<_>>>()?;
        let mut registered = BTreeSet::new();
        let mut definition_seen = false;
        let mut xor_mints = 0;
        for transaction in network.genesis().0.external_transactions() {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                continue;
            };
            for instruction in instructions {
                if let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() {
                    match register {
                        RegisterBox::AssetDefinition(value) if value.object.id == xor => {
                            assert!(!definition_seen, "one canonical XOR definition");
                            assert_eq!(
                                value.object.spec,
                                iroha_primitives::numeric::NumericSpec::fractional(9)
                            );
                            definition_seen = true;
                        }
                        RegisterBox::Account(value) if citizens.contains(&value.object.id) => {
                            assert!(registered.insert(value.object.id.clone()));
                        }
                        _ => {}
                    }
                }
                if let Some(MintBox::Asset(mint)) = instruction.as_any().downcast_ref::<MintBox>()
                    && mint.destination.definition() == &xor
                {
                    assert!(
                        definition_seen,
                        "actual builder must define XOR before using it"
                    );
                    assert!(
                        !citizens.contains(mint.destination.account()),
                        "citizens receive existing XOR only through paid owner transfers"
                    );
                    xor_mints += 1;
                }
                assert!(
                    instruction
                        .as_any()
                        .downcast_ref::<RegisterCitizen>()
                        .is_none(),
                    "citizen bonds must use funded runtime owners, not precede automatic genesis XOR bootstrap"
                );
            }
        }
        assert!(definition_seen && xor_mints >= 4);
        assert_eq!(registered, citizens);
        assert!(!defaults::governance::citizenship_bond_amount().is_zero());
        Ok(())
    }
}
