//! Native lane progress over disjoint real committees, including a stopped lane and restart.
//!
//! This fixture uses universal transaction gossip, the signed default paid fee policy, and
//! explicit original fee balances for both authorities. The SORA Nexus campaign separately
//! exercises restricted data spaces and monetary settlement.

use super::*;
use eyre::{ensure, eyre};
use iroha::data_model::{
    asset::{AssetBalancePolicy, AssetDefinition, AssetId},
    domain::Domain,
    isi::{InstructionBox, Mint, register::RegisterCommitteePeerWithPop},
    transaction::{
        DEFAULT_TRANSACTION_TIME_TO_LIVE, FeePaymentIntent, SignedTransaction, TransactionBuilder,
        TransactionPayload,
    },
};
use iroha_model_base::{domain::DomainId, peer::PeerId};
use iroha_primitives::{numeric::NumericSpec, unique_vec::UniqueVec};
use iroha_test_network::{
    CommitteeValidatorP2pBootstrap, unexecuted_genesis_factory_with_post_topology,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use std::collections::BTreeSet;

const LANES: [LaneId; 2] = [LaneId::new(2), LaneId::new(3)];
const MEMBERS: usize = 4;
const AUTHORITY_FEE_SEED: u32 = 1_000_000;
// This carrier must remain valid while the other lane commits and four stopped seats replay Kura.
const RESTART_CARRIER_TTL: Duration = Duration::from_secs(15 * 60);

fn policy(entries: &[GenesisTopologyEntry]) -> SumeragiLanePolicy {
    assert_eq!(entries.len(), LANES.len() * MEMBERS);
    let mut policy = SumeragiLanePolicy::for_chain(
        SumeragiParameters::default(),
        iroha_sumeragi::availability::recommended_data_availability_layout(),
    );
    policy.stall_window = 10_000;
    policy.fixed = entries
        .as_chunks::<MEMBERS>()
        .0
        .iter()
        .zip(LANES)
        .map(|(entries, lane)| {
            let mut committee = entries
                .iter()
                .map(|entry| SumeragiLaneMember {
                    peer: entry.peer.clone(),
                    pop: entry.pop_bytes().unwrap().expect("participant PoP"),
                })
                .collect::<Vec<_>>();
            committee.sort_by(|left, right| left.peer.cmp(&right.peer));
            SumeragiFixedLane {
                lane,
                dataspace: DataSpaceId::UNIVERSAL,
                committee,
            }
        })
        .collect();
    policy.routes = LANES
        .into_iter()
        .zip([&*ALICE_ID, &*BOB_ID])
        .map(|(lane, account)| SumeragiLaneRoute {
            lane,
            account: Some(account.canonical_i105().expect("canonical authority")),
            instruction: Some("Log".to_owned()),
        })
        .collect();
    policy.validate().expect("disjoint native lane policy");
    policy
}

fn disjoint_builder() -> Result<NetworkBuilder> {
    Ok(NetworkBuilder::new()
        .with_peers(MEMBERS)
        .with_committee_validator_p2p_bootstrap(CommitteeValidatorP2pBootstrap::new(
            LANES.len() * MEMBERS,
        )?)?
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            // Retain the global builder's merge selection and retry reason across peer restarts.
            layer.write(
                ["logger", "filter"],
                "info,iroha_core::sumeragi::executor=debug,iroha_core::sumeragi::driver::exec=debug",
            );
            layer.write(
                ["nexus", "storage", "local_budget_bytes"],
                TEST_NEXUS_LOCAL_STORAGE_BUDGET_BYTES,
            );
        })
        .with_genesis_block_and_committee_validator_entries(disjoint_genesis))
}

/// Custom genesis owns its resources: the builder does not add default fee balances here.
fn disjoint_genesis(
    global: UniqueVec<PeerId>,
    voters: Vec<GenesisTopologyEntry>,
    participants: Vec<GenesisTopologyEntry>,
) -> iroha_genesis::GenesisBlock {
    let fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    let staking = iroha_genesis::default_staking_policy(
        fees.fee_asset_id.clone(),
        fees.fee_sink_account_id.clone(),
        fees.fee_sink_account_id.clone(),
    );
    let mut funded = vec![
        SetParameter::new(Parameter::Custom(
            fees.clone()
                .into_custom_parameter()
                .expect("signed paid fee policy"),
        ))
        .into(),
        Register::domain(Domain::new(
            DomainId::try_new("universal", "universal").expect("fee asset domain"),
        ))
        .into(),
        Register::asset_definition(AssetDefinition::new(
            fees.fee_asset_id.clone(),
            "XOR".to_owned(),
            NumericSpec::fractional(9),
            AssetBalancePolicy::Global,
            None,
        ))
        .into(),
    ];
    funded.extend([ALICE_ID.clone(), BOB_ID.clone()].map(|account| {
        Mint::asset_quantity(
            AUTHORITY_FEE_SEED,
            AssetId::new(fees.fee_asset_id.clone(), account),
        )
        .into()
    }));
    let mut lane_instructions = participants
        .iter()
        .map(|entry| {
            RegisterCommitteePeerWithPop::new(
                entry.peer.clone(),
                entry.pop_bytes().unwrap().expect("participant PoP"),
            )
            .into()
        })
        .collect::<Vec<InstructionBox>>();
    lane_instructions.push(
        SetParameter::new(Parameter::Custom(
            policy(&participants).into_custom_parameter(),
        ))
        .into(),
    );
    unexecuted_genesis_factory_with_post_topology(
        vec![funded],
        vec![lane_instructions],
        global,
        voters,
        staking,
    )
}

/// Check signer ownership on the original observed processes, including global lane observers.
fn ready_frontiers(network: &Network, peers: &[&NetworkPeer]) -> Result<[u64; 2]> {
    ensure!(!peers.is_empty(), "no peers selected for lane progress");
    let mut minimum = [u64::MAX; 2];
    let mut maximum = [0; 2];
    for peer in peers {
        let key = peer
            .bls_public_key()
            .ok_or_else(|| eyre!("missing BLS key"))?;
        let statuses = peer.client().client().get_sumeragi_lanes()?;
        ensure!(statuses.len() == LANES.len(), "unexpected lane population");
        for (ordinal, lane) in LANES.into_iter().enumerate() {
            let status = statuses
                .iter()
                .find(|status| status.record.lane == lane)
                .ok_or_else(|| eyre!("missing lane {lane}"))?;
            let instance = status
                .instance
                .as_ref()
                .ok_or_else(|| eyre!("lane is not running"))?;
            ensure!(instance.halted.is_none(), "lane {lane} halted");
            let expected = network.committee_validators()
                [ordinal * MEMBERS..(ordinal + 1) * MEMBERS]
                .iter()
                .map(|member| PeerId::new(member.bls_public_key().unwrap().clone()))
                .collect::<BTreeSet<_>>();
            ensure!(
                status
                    .record
                    .committee
                    .iter()
                    .map(|member| member.peer.clone())
                    .collect::<BTreeSet<_>>()
                    == expected,
                "lane {lane} changed its pinned disjoint committee"
            );
            let is_member = expected.contains(&PeerId::new(key.clone()));
            ensure!(
                instance.signer.as_ref() == is_member.then_some(key),
                "wrong lane signer ownership"
            );
            minimum[ordinal] = minimum[ordinal].min(status.record.merged.height);
            maximum[ordinal] = maximum[ordinal].max(status.record.merged.height);
        }
    }
    ensure!(
        minimum == maximum,
        "peers have not converged on lane frontiers"
    );
    Ok(minimum)
}

fn wait_frontiers(network: &Network, at_least: [u64; 2]) -> Result<[u64; 2]> {
    wait_frontiers_with_stopped(network, at_least, &[])
}

/// Omit only the explicitly stopped seats, never a process that unexpectedly exited.
fn wait_frontiers_with_stopped(
    network: &Network,
    at_least: [u64; 2],
    stopped: &[NetworkPeer],
) -> Result<[u64; 2]> {
    let peers: Vec<_> = network
        .all_peers()
        .filter(|peer| !stopped.iter().any(|stopped| stopped.id() == peer.id()))
        .collect();
    ensure!(!peers.is_empty(), "no peers selected for lane progress");
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        match ready_frontiers(network, &peers) {
            Ok(frontiers)
                if frontiers
                    .iter()
                    .zip(at_least)
                    .all(|(actual, required)| *actual >= required) =>
            {
                return Ok(frontiers);
            }
            observation if Instant::now() >= deadline => {
                bail!("disjoint lane progress failed: {observation:?}; required {at_least:?}")
            }
            _ => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

fn lane_work_payload(
    network: iroha_data_model::NetworkId,
    authority: &iroha_data_model::AccountId,
    message: &str,
    ttl: Duration,
) -> Result<TransactionPayload> {
    let mut builder = TransactionBuilder::new(
        network,
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_ttl(ttl);
    Ok(builder
        .with_instructions([Log::new(Level::INFO, message.to_owned())])
        .into_payload()?)
}

fn submit_lane_work(
    network: &Network,
    ordinal: usize,
    message: &str,
    ttl: Duration,
) -> Result<SignedTransaction> {
    let (authority, key) = match ordinal {
        0 => (&*ALICE_ID, &*ALICE_KEYPAIR),
        1 => (&*BOB_ID, &*BOB_KEYPAIR),
        _ => bail!("unknown fixture lane"),
    };
    let client = running_peer(network)?.client_for(authority, key.private_key().clone());
    let payload = lane_work_payload(network.network_id(), authority, message, ttl)?;
    let quote = client
        .quote_fees(iroha::client::FeeQuoteRequest::AccountSignature { payload: &payload })?;
    let transaction = sign_quoted_lane_work(payload, quote.intent, key.private_key())?;
    // Enter through a global validator outside both lane committees. The production queue,
    // gossip, lane consensus and global merge must carry the original signed transaction.
    // Admission remains separate from finality so stopped-lane work can remain pending.
    client.submit_transaction(&transaction)?;
    eprintln!(
        "admitted lane {} transaction {} ({message}), created {} ms, lifetime {} ms",
        LANES[ordinal],
        transaction.hash(),
        transaction.creation_time().as_millis(),
        ttl.as_millis(),
    );
    Ok(transaction)
}

/// Sign only the original payload plus the quote's exact payer-preserving fee limits.
fn sign_quoted_lane_work(
    mut payload: TransactionPayload,
    quoted: FeePaymentIntent,
    key: &iroha_crypto::PrivateKey,
) -> Result<SignedTransaction> {
    ensure!(
        payload.fee_payment.has_same_payer_and_gas_bound(&quoted),
        "lane fee quote changed the selected payer or gas bound"
    );
    payload.fee_payment = quoted;
    Ok(TransactionBuilder::from_payload(payload)?.sign(key))
}

#[test]
fn disjoint_lane_committees_keep_progress_when_one_stops_and_all_restart() -> Result<()> {
    init_instruction_registry();
    let Some((network, rt)) = sandbox::start_network_blocking_or_skip(
        disjoint_builder()?,
        stringify!(disjoint_lane_committees_keep_progress_when_one_stops_and_all_restart),
    )?
    else {
        return Ok(());
    };
    let result = (|| -> Result<()> {
        ensure!(
            network.validators().len() == MEMBERS
                && network.committee_validators().len() == 2 * MEMBERS,
            "global quorum must remain exactly four"
        );
        register_account_everywhere(&network)?;
        register_account_everywhere(&network)?;
        wait_frontiers(&network, [0, 0])?;
        submit_lane_work(
            &network,
            0,
            "first disjoint lane",
            DEFAULT_TRANSACTION_TIME_TO_LIVE,
        )?;
        submit_lane_work(
            &network,
            1,
            "second disjoint lane",
            DEFAULT_TRANSACTION_TIME_TO_LIVE,
        )?;
        let before = wait_frontiers(&network, [1, 1])?;
        for peer in &network.committee_validators()[..MEMBERS] {
            rt.block_on(peer.shutdown());
        }
        let stalled = submit_lane_work(
            &network,
            0,
            "retained while its entire committee is stopped",
            RESTART_CARRIER_TTL,
        )?;
        submit_lane_work(
            &network,
            1,
            "continues with the other lane stopped",
            DEFAULT_TRANSACTION_TIME_TO_LIVE,
        )?;
        let after = wait_frontiers_with_stopped(
            &network,
            [before[0], before[1] + 1],
            &network.committee_validators()[..MEMBERS],
        )?;
        // Already certified lane blocks may still merge after shutdown. The new carrier
        // submitted after every member stopped must remain pending while the other lane runs.
        ensure!(
            matches!(
                running_peer(&network)?
                    .client()
                    .client()
                    .get_transaction_status(stalled.hash())?,
                None | Some(
                    iroha::client::TxConfirmationStatus::Queued
                        | iroha::client::TxConfirmationStatus::Approved(_)
                )
            ),
            "fresh work finalized or failed while its entire lane committee was stopped"
        );
        for peer in &network.committee_validators()[..MEMBERS] {
            let layers: Vec<_> = network.config_layers_for_peer(peer).collect();
            rt.block_on(peer.start_checked(layers.iter(), None))?;
        }
        let recovered = wait_frontiers(&network, [after[0] + 1, after[1]])?;
        // Finality is for the exact original pending carrier, with no replay submission.
        running_peer(&network)?
            .client()
            .client()
            .wait_for_transaction_applied(
                stalled.hash(),
                iroha::client::TransactionWaitOptions::default(),
            )?;
        let height = committed_height(&network)?;
        for peer in network.all_peers() {
            rt.block_on(peer.shutdown());
        }
        for peer in network.all_peers() {
            let layers: Vec<_> = network.config_layers_for_peer(peer).collect();
            rt.block_on(peer.start_checked(layers.iter(), None))?;
        }
        wait_for_committed(&network, height, Duration::from_secs(120))?;
        ensure!(
            wait_frontiers(&network, recovered)? == recovered,
            "restart changed a certified lane frontier"
        );
        submit_lane_work(
            &network,
            0,
            "first lane after all-seat restart",
            DEFAULT_TRANSACTION_TIME_TO_LIVE,
        )?;
        submit_lane_work(
            &network,
            1,
            "second lane after all-seat restart",
            DEFAULT_TRANSACTION_TIME_TO_LIVE,
        )?;
        wait_frontiers(&network, [recovered[0] + 1, recovered[1] + 1])?;
        Ok(())
    })();
    rt.block_on(network.shutdown());
    result
}

#[test]
fn disjoint_original_genesis_funds_both_authorities_under_exact_paid_policy() {
    use iroha_data_model::{
        Identifiable,
        isi::{MintBox, RegisterBox},
    };
    init_instruction_registry();
    let entries = (0..(MEMBERS * 3))
        .map(|index| {
            let key = iroha_crypto::KeyPair::try_from_seed(
                vec![u8::try_from(index + 1).unwrap(); 32],
                iroha_crypto::Algorithm::BlsNormal,
            )
            .expect("deterministic disjoint fixture key");
            GenesisTopologyEntry::new(
                PeerId::new(key.public_key().clone()),
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("genuine fixture PoP"),
            )
        })
        .collect::<Vec<_>>();
    let global = entries[..MEMBERS].to_vec();
    let participants = entries[MEMBERS..].to_vec();
    let expected_lanes = policy(&participants);
    let genesis = disjoint_genesis(
        global.iter().map(|entry| entry.peer.clone()).collect(),
        global,
        participants,
    );
    iroha_data_model::sumeragi_finality::authenticated_genesis(&genesis.0)
        .expect("the original signed topology, policy and fee resources authenticate");
    let fees = iroha_genesis::signed_genesis_root_fee_policy(&genesis.0).unwrap();
    assert_eq!(
        fees,
        iroha_config::parameters::defaults::nexus::fees::genesis_policy()
    );
    assert!(!fees.per_instruction_fee.is_zero());
    assert!(!fees.per_gas_unit_fee.is_zero());
    let mut definitions = Vec::new();
    let mut balances = Vec::new();
    let mut lanes = Vec::new();
    let mut committee_registrations = Vec::new();
    for transaction in genesis.0.external_transactions() {
        for instruction in transaction.instructions().explicit_instructions() {
            if let Some(RegisterBox::AssetDefinition(register)) =
                instruction.as_any().downcast_ref::<RegisterBox>()
            {
                if register.object().id() == &fees.fee_asset_id {
                    definitions.push(register.object().clone());
                }
            }
            if let Some(MintBox::Asset(mint)) = instruction.as_any().downcast_ref::<MintBox>() {
                if mint.destination.definition() == &fees.fee_asset_id {
                    balances.push((mint.destination.clone(), mint.object.clone()));
                }
            }
            if let Some(parameter) = instruction.as_any().downcast_ref::<SetParameter>() {
                if let Parameter::Custom(custom) = parameter.inner() {
                    if custom.id() == &SumeragiLanePolicy::parameter_id() {
                        lanes.push(
                            SumeragiLanePolicy::from_custom_parameter(custom)
                                .unwrap()
                                .unwrap(),
                        );
                    }
                }
            }
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<RegisterCommitteePeerWithPop>()
            {
                committee_registrations.push(register.peer.clone());
            }
        }
    }
    assert_eq!(definitions.len(), 1, "one original fee definition");
    let mut expected = [ALICE_ID.clone(), BOB_ID.clone()]
        .map(|account| {
            (
                AssetId::new(fees.fee_asset_id.clone(), account),
                AUTHORITY_FEE_SEED.into(),
            )
        })
        .to_vec();
    balances.sort();
    expected.sort();
    assert_eq!(
        balances, expected,
        "both original fee balances are explicitly funded"
    );
    assert_eq!(lanes, vec![expected_lanes.clone()]);
    let actual: BTreeSet<_> = committee_registrations.iter().cloned().collect();
    let expected: BTreeSet<_> = expected_lanes
        .fixed
        .iter()
        .flat_map(|lane| lane.committee.iter().map(|member| member.peer.clone()))
        .collect();
    assert_eq!(committee_registrations.len(), 2 * MEMBERS);
    assert_eq!(
        actual, expected,
        "funding leaves all eight lane participants intact"
    );
}

#[test]
fn lane_quote_signing_preserves_payload_and_rejects_payer_or_gas_changes() -> Result<()> {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        nexus::FeeSponsorProgramId,
        transaction::{FeeChargeKind, FeeChargeLimit},
    };
    let network = iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        Hash::new(b"pure disjoint quote binding"),
    ));
    let fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    for (authority, key) in [(&*ALICE_ID, &*ALICE_KEYPAIR), (&*BOB_ID, &*BOB_KEYPAIR)] {
        let ttl = if authority == &*ALICE_ID {
            RESTART_CARRIER_TTL
        } else {
            DEFAULT_TRANSACTION_TIME_TO_LIVE
        };
        let payload = lane_work_payload(network, authority, "exact lane carrier", ttl)?;
        let quoted = FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                fees.fee_asset_id.clone(),
                "0.001".parse()?,
            )],
            None,
        );
        let signed = sign_quoted_lane_work(payload.clone(), quoted.clone(), key.private_key())?;
        let mut expected = payload.clone();
        expected.fee_payment = quoted.clone();
        assert_eq!(signed.payload(), &expected);
        assert_eq!(signed.time_to_live(), Some(ttl));
        signed.verify_signature()?;
        assert!(
            sign_quoted_lane_work(
                payload.clone(),
                FeePaymentIntent::authority(
                    quoted.charge_limits().to_vec(),
                    std::num::NonZeroU64::new(1)
                ),
                key.private_key()
            )
            .is_err()
        );
        assert!(
            sign_quoted_lane_work(
                payload,
                FeePaymentIntent::sponsor(
                    FeeSponsorProgramId::new(BOB_ID.clone(), "unexpected".parse()?),
                    1,
                    quoted.charge_limits().to_vec(),
                    None
                ),
                key.private_key()
            )
            .is_err()
        );
    }
    Ok(())
}
