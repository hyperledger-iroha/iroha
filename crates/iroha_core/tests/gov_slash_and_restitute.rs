//! Governance slashing and restitution flows for plain ballots and manual appeals.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_core::{
    governance::manifest::LaneManifestRegistry,
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_data_model::{
    Registrable,
    asset::{Asset, AssetDefinition},
    block::BlockHeader,
    domain::Domain,
    events::data::governance::GovernanceSlashReason,
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
    transaction::{
        FeePaymentIntent, TransactionBuilder, TransactionEntrypoint,
        signed::{
            SealedTransactionCommitmentPayload, SealedTransactionReveal,
            SignedSealedTransactionCommitment, compute_sealed_transaction_commitment,
        },
    },
};
use iroha_executor_data_model::permission::governance::{
    CanRestituteGovernanceLock, CanSlashGovernanceLock, CanSubmitGovernanceBallot,
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, gen_account_in};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
use std::{borrow::Cow, sync::Arc};
fn governance_world_with_accounts(
    voting_asset_id: AssetDefinitionId,
    escrow_account: &iroha_data_model::account::AccountId,
    slash_account: &iroha_data_model::account::AccountId,
) -> World {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id.clone()).build(escrow_account);
    let alice_account =
        iroha_data_model::account::Account::new(ALICE_ID.clone()).build(escrow_account);
    let escrow =
        iroha_data_model::account::Account::new(escrow_account.clone()).build(escrow_account);
    let slash =
        iroha_data_model::account::Account::new(slash_account.clone()).build(escrow_account);
    let asset_def = AssetDefinition::numeric(
        voting_asset_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(escrow_account);
    // Seed balances: Alice 1_000, escrow 0, slash 0.
    let alice_asset = Asset::new(
        AssetId::new(voting_asset_id.clone(), ALICE_ID.clone()),
        Quantity::from(1_000_u64),
    );
    let escrow_asset = Asset::new(
        AssetId::new(voting_asset_id.clone(), escrow_account.clone()),
        Quantity::from(0_u64),
    );
    let slash_asset = Asset::new(
        AssetId::new(voting_asset_id, slash_account.clone()),
        Quantity::from(0_u64),
    );
    World::with_assets(
        [domain],
        [alice_account, escrow, slash],
        [asset_def],
        [alice_asset, escrow_asset, slash_asset],
        [],
    )
}
fn governance_state_with_accounts(
    voting_asset_id: AssetDefinitionId,
    escrow_account: &iroha_data_model::account::AccountId,
    slash_account: &iroha_data_model::account::AccountId,
) -> State {
    let world = governance_world_with_accounts(voting_asset_id, escrow_account, slash_account);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    State::new_for_testing(world, kura, query_handle)
}
/// Configure exact fixture policy and the real four-validator public-lane manifest.
fn configure_retained_governance_state(
    state: &mut State,
    voting_asset: &AssetDefinitionId,
    escrow: &iroha_data_model::account::AccountId,
    slash: &iroha_data_model::account::AccountId,
    keys: &[iroha_crypto::KeyPair],
) {
    let mut governance = state.gov.clone();
    governance.plain_voting_enabled = true;
    governance.voting_asset_id = voting_asset.clone();
    governance.min_bond_amount = 10_u64.into();
    governance.bond_escrow_account = escrow.clone();
    governance.slash_receiver_account = slash.clone();
    governance.slash_double_vote_bps = 2_000;
    state.set_gov(governance);
    let lane = state.nexus_snapshot().lane_catalog.lanes()[0].clone();
    let validators = keys
        .iter()
        .map(|key| iroha_data_model::account::AccountId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let bindings = validators
        .iter()
        .zip(keys)
        .map(
            |(account, key)| crate::governance::manifest::ManifestValidatorBinding {
                validator: account.clone(),
                peer_id: iroha_model_base::peer::PeerId::new(key.public_key().clone()),
                torii_url: None,
            },
        )
        .collect();
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(
        std::collections::BTreeMap::from([(
            lane.id,
            crate::governance::manifest::LaneManifestStatus {
                lane: lane.id,
                alias: lane.alias,
                dataspace: lane.dataspace_id,
                visibility: lane.visibility,
                storage: lane.storage,
                governance: lane.governance,
                manifest_path: Some(std::path::PathBuf::from(
                    "fixtures/governance-retained-manifest.json",
                )),
                governance_rules: Some(crate::governance::manifest::GovernanceRules {
                    validators,
                    validator_bindings: bindings,
                    ..crate::governance::manifest::GovernanceRules::default()
                }),
                privacy_commitments: Vec::new(),
            },
        )]),
    )));
}

/// Construct the final signed genesis and exact-network State before any candidate executes.
fn retained_governance_fixture(
    voting_asset: &AssetDefinitionId,
    escrow: &iroha_data_model::account::AccountId,
    slash: &iroha_data_model::account::AccountId,
) -> (
    State,
    iroha_data_model::block::SignedBlock,
    iroha_data_model::block::consensus_v2::HeightContext,
    Vec<iroha_crypto::KeyPair>,
) {
    use iroha_data_model::block::consensus_v2::{
        ConsensusMode, SumeragiV2GenesisContextParameters, ValidatorPower,
    };
    use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
    use iroha_model_base::peer::PeerId;
    use norito::codec::Encode;
    iroha_genesis::init_instruction_registry();
    let mut keys = (1_u8..=4)
        .map(|seed| {
            iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal)
                .unwrap()
        })
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let roster = keys
        .iter()
        .map(|key| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let mut configured = governance_state_with_accounts(voting_asset.clone(), escrow, slash);
    configure_retained_governance_state(&mut configured, voting_asset, escrow, slash, &keys);
    // This is a pre-sign configuration projection, not a candidate execution or
    // a replacement validated State. The actual prepared genesis is frozen and
    // checked against it before any finality is signed or published.
    let mut parameters = SumeragiV2GenesisContextParameters::recommended();
    {
        let projection = configured.block(BlockHeader::new(nonzero!(1_u64), None, None, 1_000, 0));
        parameters.nexus_amx_context_hash =
            crate::sumeragi::staged_genesis_nexus_amx_context_hash(&projection).into();
        parameters.execution_policy_hash =
            crate::sumeragi::staged_genesis_execution_policy_hash(&projection)
                .unwrap()
                .into();
    }
    let chain_id = configured.view().chain_id.clone();
    let features = {
        let view = configured.view();
        crate::state::compute_confidential_feature_digest(
            view.world(),
            &view.zk,
            view.sccp_registry.as_ref(),
            1,
        )
    };
    let nexus = configured.nexus_snapshot();
    let genesis = GenesisBuilder::new_without_executor(chain_id.clone(), ".")
        .with_sumeragi_v2_context_parameters(parameters)
        .with_kagemusha_mint_finality_genesis_parameters(
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_parameters(&roster),
        )
        .set_topology(
            keys.iter()
                .map(|key| {
                    GenesisTopologyEntry::new(
                        PeerId::new(key.public_key().clone()),
                        iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
                    )
                })
                .collect(),
        )
        .build_raw()
        .unwrap()
        .with_consensus_meta()
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash_at(
            &ALICE_KEYPAIR,
            Some(crate::da::active_proof_policy_bundle_at_height(&nexus, 1)),
            features.zk_policy_hash,
            1_000,
        )
        .expect("sign final four-validator RS16 governance genesis")
        .0;
    let network = iroha_data_model::NetworkId::from_genesis_hash(genesis.hash());
    drop(configured);
    let mut state = State::new_with_chain_and_network_id_for_testing(
        governance_world_with_accounts(voting_asset.clone(), escrow, slash),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        chain_id,
        network,
    );
    configure_retained_governance_state(&mut state, voting_asset, escrow, slash, &keys);
    let metadata = iroha_genesis::signed_genesis_consensus_metadata(&genesis).unwrap();
    assert_eq!(
        ConsensusMode::from(metadata.mode),
        ConsensusMode::Permissioned
    );
    let mut seed = b"sumeragi-v2:permissioned-leader-seed".to_vec();
    seed.extend_from_slice(&network.encode());
    let context = crate::sumeragi::v2_context::build_genesis_height_context(
        crate::sumeragi::v2_context::GenesisContextInputs {
            network_id: network,
            election: crate::sumeragi::v2_context::FrozenElectionInputs {
                epoch: 0,
                kagemusha_mint_finality_epoch_roster: metadata
                    .kagemusha_mint_finality
                    .epoch_roster
                    .bind_network_id(network)
                    .unwrap(),
                epoch_end_height: u64::MAX,
                mode: ConsensusMode::Permissioned,
                roster,
                leader_seed: iroha_crypto::Hash::new(seed).into(),
            },
            next_epoch_snapshot: None,
            nexus_amx_context_hash: iroha_crypto::Hash::prehashed(
                metadata.sumeragi_v2.nexus_amx_context_hash,
            ),
            execution_policy_hash: iroha_crypto::Hash::prehashed(
                metadata.sumeragi_v2.execution_policy_hash,
            ),
            da_layout: metadata.sumeragi_v2.da_layout,
        },
    )
    .unwrap();
    (state, genesis, context, keys)
}

/// Run the canonical candidate producer once, retaining every resulting journal.
fn prepare_retained_governance_candidate<'state>(
    state: &'state State,
    proposal: iroha_data_model::block::SignedBlock,
    context: &iroha_data_model::block::consensus_v2::HeightContext,
    executions: &mut usize,
) -> crate::state::PreparedCarrier<'state> {
    *executions += 1;
    let topology = crate::sumeragi::network_topology::Topology::new(
        context.roster.iter().map(|member| member.validator.clone()),
    );
    let signature_policy = if proposal.header().is_genesis() {
        crate::sumeragi::v2_body_store::BlockSignaturePolicy::GenesisAuthority(
            ALICE_KEYPAIR.public_key().clone(),
        )
    } else {
        crate::sumeragi::v2_body_store::BlockSignaturePolicy::RotatingLeader
    };
    crate::sumeragi::v2_body_store::verify_origin_block_signature(
        context,
        &proposal,
        &signature_policy,
    )
    .expect("authenticate actual immutable origin-view signature before execution");
    let clock = iroha_primitives::time::TimeSource::new_fixed(proposal.header().creation_time());
    crate::block::ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block(
        proposal,
        &topology,
        &ALICE_ID,
        &clock,
        state.sumeragi_block_cadence(),
        crate::block::valid::SumeragiV2ValidationContext::from_height_context(context),
        state,
        &mut None,
    )
    .unwrap_or_else(|(_, error)| panic!("prepare original governance candidate: {error}"))
}

/// Derive the successor from actual predecessor finality and authoritative routing.
fn retained_governance_successor(
    state: &State,
    parent: &iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
    entrypoint: TransactionEntrypoint,
    keys: &[iroha_crypto::KeyPair],
) -> (
    iroha_data_model::block::SignedBlock,
    iroha_data_model::block::consensus_v2::HeightContext,
) {
    let context = crate::sumeragi::v2_context::build_successor_height_context(
        parent,
        parent.height_context.nexus_amx_context_hash,
        None,
    )
    .unwrap();
    assert_eq!(context.parent_commit_qc.as_ref(), Some(&parent.commit_qc));
    assert_eq!(state.latest_block_hash_fast(), Some(parent.block_hash));
    let (events, _receiver) = tokio::sync::broadcast::channel(32);
    let queue = crate::queue::Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events,
    );
    let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
    let route = queue.route_plan_with_state(&accepted, state).unwrap();
    let leader = context.leader(0);
    let plan = crate::sumeragi::lane_planner::prepare_v2_lane_payload_plan(
        state,
        state.kura(),
        &context,
        0,
        &context.roster[leader as usize].validator,
        &[route.coordinator_route()],
        &[iroha_crypto::Hash::from(entrypoint.hash())],
    )
    .unwrap();
    assert!(
        plan.unavailable_indices.is_empty(),
        "successor at height {} retains unavailable candidate indices {:?}; ordinary frontier {:?}; certified frontier {:?}; Native AMX frontier {:?}",
        context.height,
        plan.unavailable_indices,
        state.unapplied_lane_block_artifact_heights_snapshot_cached(),
        state.unapplied_certified_lane_block_heights_snapshot_cached(),
        state.unapplied_native_amx_participant_control_heights_snapshot()
    );
    let execution_context = iroha_data_model::block::BlockExecutionContextBundle::new(vec![
        crate::queue::execution_context_for_routing_plan(entrypoint.hash(), &route),
    ])
    .with_lane_payload_ownerships(plan.ownerships);
    let previous = state
        .kura()
        .get_block(std::num::NonZeroUsize::new((context.height - 1) as usize).unwrap())
        .unwrap();
    let creation_time = previous.header().creation_time() + state.sumeragi_block_cadence();
    let mut header = BlockHeader::new(
        std::num::NonZeroU64::new(context.height).unwrap(),
        Some(parent.block_hash),
        None,
        creation_time.as_millis().try_into().unwrap(),
        0,
    );
    let features = {
        let view = state.view();
        crate::state::compute_confidential_feature_digest(
            view.world(),
            &view.zk,
            view.sccp_registry.as_ref(),
            context.height,
        )
    };
    header.set_confidential_features((!features.is_empty()).then_some(features));
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    match entrypoint {
        TransactionEntrypoint::SealedCommitment(value) => {
            builder.push_sealed_transaction_commitment(value);
        }
        TransactionEntrypoint::SealedReveal(value) => {
            builder.push_sealed_transaction_reveal(value);
        }
        _ => panic!("governance fixture expects exact sealed carriers"),
    }
    builder.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
        &state.nexus_snapshot(),
        context.height,
    )));
    builder.set_execution_context(Some(execution_context));
    let signer = keys
        .iter()
        .find(|key| key.public_key() == context.roster[leader as usize].validator.public_key())
        .unwrap();
    let proposal = builder
        .try_build_with_signature(u64::from(leader), signer.private_key())
        .unwrap()
        .canonical_resultless_proposal();
    (proposal, context)
}

fn seed_slash_snapshot(
    state: &mut State,
    rid: &str,
    escrow_asset_id: &AssetId,
    slash_asset_id: &AssetId,
) {
    let mut seed_block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let mut seed_tx = seed_block.transaction();
    iroha_core::query::standalone_plain_test_fixture::fund_voter(
        &mut seed_tx,
        &ALICE_ID,
        1_000_000_u64.into(),
        0,
    );
    let mut snapshot_governance = seed_tx.gov.clone();
    snapshot_governance.conviction_step_blocks = 1;
    seed_tx.world.governance_referenda_mut().insert(
        rid.to_owned(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 1,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Open,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
            plain_context: iroha_core::query::standalone_plain_test_fixture::context(
                &snapshot_governance,
                0,
            ),
            plain_result: iroha_data_model::governance::conviction::PlainVotingResultV1::Pending,
        },
    );
    let mut locks = iroha_core::state::GovernanceLocksForReferendum::default();
    locks.locks.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: ALICE_ID.clone(),
            amount: 60_u64.into(),
            slashed: 40_u64.into(),
            expiry_height: 100,
            direction: 0,
            duration_blocks: 99,
            custody: iroha_core::state::GovernanceLockCustody {
                escrowed: true,
                asset_definition_id: escrow_asset_id.definition().clone(),
                bond_escrow_account: escrow_asset_id.account().clone(),
                slash_receiver_account: slash_asset_id.account().clone(),
            },
        },
    );
    seed_tx
        .world
        .governance_locks_mut()
        .insert(rid.to_string(), locks);
    let mut ledger = iroha_core::state::GovernanceSlashLedger::default();
    ledger.slashes.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceSlashEntry {
            total_slashed: 40_u64.into(),
            total_restituted: 0_u64.into(),
            last_reason: GovernanceSlashReason::DoubleVote,
            last_height: 1,
        },
    );
    seed_tx
        .world
        .governance_slashes_mut()
        .insert(rid.to_string(), ledger);
    **seed_tx
        .world
        .asset_mut(escrow_asset_id)
        .expect("escrow asset") = Quantity::from(60_u64);
    **seed_tx
        .world
        .asset_mut(slash_asset_id)
        .expect("slash asset") = Quantity::from(40_u64);
    seed_tx.apply();
    seed_block
        .commit_empty_block_for_testing()
        .expect("commit slash snapshot");
}
#[test]
#[allow(clippy::too_many_lines)]
fn double_vote_slashes_plain_lock() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let (escrow_id, _) = gen_account_in("wonderland");
    let (slash_id, _) = gen_account_in("wonderland");
    let (state, genesis, context, keys) =
        retained_governance_fixture(&def_id, &escrow_id, &slash_id);
    let alice = ALICE_ID.clone();
    let mut executions = 0;
    let mut publications = 0;
    // Explicit pre-genesis World fixture: fund and cast the initial ballot.
    // This is not a claim that those direct calls were signed genesis intents.
    let rid = "rid-slash-plain".to_string();
    {
        // The direct ballot fixture publishes native transfer transcripts.
        let fixture_witness_guard = iroha_core::sumeragi::witness::exec_witness_guard();
        // This is explicit test world setup, not a finalized genesis output.
        // Signed genesis executes separately against the resulting funded world.
        let mut sblock1 = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
        let mut stx1 = sblock1.transaction();
        iroha_core::query::standalone_plain_test_fixture::fund_voter(
            &mut stx1,
            &iroha_test_samples::ALICE_ID,
            1_000_000_u64.into(),
            0,
        );
        stx1.world.governance_referenda_mut().insert(
            rid.clone(),
            iroha_core::state::GovernanceReferendumRecord {
                h_start: 1,
                h_end: 50,
                status: iroha_core::state::GovernanceReferendumStatus::Open,
                mode: iroha_core::state::GovernanceReferendumMode::Plain,
                plain_context: iroha_core::query::standalone_plain_test_fixture::context(
                    &stx1.gov, 0,
                ),
                plain_result:
                    iroha_data_model::governance::conviction::PlainVotingResultV1::Pending,
            },
        );
        let perm: Permission = CanSubmitGovernanceBallot {
            referendum_id: rid.clone(),
        }
        .into();
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx1)
            .expect("grant ballot permission");
        let ballot_ok = iroha_data_model::isi::governance::CastPlainBallot {
            referendum_id: rid.clone(),
            direction: 0,
            owner: ALICE_ID.clone(),
            amount: 20_u64.into(),
            duration_blocks: 200,
        };
        ballot_ok
            .execute(&ALICE_ID, &mut stx1)
            .expect("first ballot should succeed");
        stx1.apply();
        sblock1
            .commit_world_overlay_for_testing()
            .expect("retain direct funded governance fixture world without block history");
        // Signed-block validation acquires its own non-reentrant recorder guard.
        drop(fixture_witness_guard);
        assert!(state.view().block_hashes().is_empty());
    }
    let prepared =
        prepare_retained_governance_candidate(&state, genesis, &context, &mut executions);
    assert!(
        prepared
            .block()
            .output_results()
            .all(|result| result.is_ok())
    );
    let finality = crate::state::publish_governance_fixture(&state, prepared, &keys);
    publications += 1;
    // Block 2: commit the sealed carrier for the conflicting ballot.
    let ballot_conflict = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: rid.clone(),
        direction: 1,
        owner: ALICE_ID.clone(),
        amount: 30_u64.into(),
        duration_blocks: 200,
    };
    let mut transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(std::time::Duration::from_millis(1_001));
    let transaction = transaction
        .with_instructions([ballot_conflict])
        .sign(ALICE_KEYPAIR.private_key());
    let salt = [0xA5; 32];
    let reveal_deadline_height = 10;
    let commitment = compute_sealed_transaction_commitment(
        state.network_id_ref(),
        &transaction,
        salt,
        reveal_deadline_height,
    );
    let sealed_commitment = SignedSealedTransactionCommitment::sign(
        SealedTransactionCommitmentPayload::new(
            *state.network_id_ref(),
            ALICE_ID.clone(),
            commitment,
            3,
            reveal_deadline_height,
            None,
        ),
        ALICE_KEYPAIR.private_key(),
    );
    let commitment_entrypoint = TransactionEntrypoint::SealedCommitment(sealed_commitment);
    let commitment_hash = commitment_entrypoint.hash();
    let (proposal, context) =
        retained_governance_successor(&state, &finality, commitment_entrypoint, &keys);
    let prepared =
        prepare_retained_governance_candidate(&state, proposal, &context, &mut executions);
    prepared
        .block()
        .validate_output_merkle_cache()
        .expect("complete commitment outputs");
    assert_eq!(
        prepared.block().network_input_hashes().collect::<Vec<_>>(),
        [commitment_hash]
    );
    let (_, commitment_output) = prepared
        .block()
        .network_output_at(0)
        .expect("exact commitment Network output");
    assert!(
        commitment_output.result.is_ok(),
        "sealed commitment must be retained before reveal: {:?}",
        commitment_output.result
    );
    let finality = crate::state::publish_governance_fixture(&state, prepared, &keys);
    publications += 1;

    // Block 3: the sealed reveal enters the shared sequential corridor. The
    // ballot remains rejected while its prevalidated slash commits separately.
    let reveal_entrypoint = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        commitment,
        transaction.clone(),
        salt,
    ));
    let reveal_hash = reveal_entrypoint.hash();
    let (proposal, context) =
        retained_governance_successor(&state, &finality, reveal_entrypoint, &keys);
    let prepared =
        prepare_retained_governance_candidate(&state, proposal, &context, &mut executions);
    prepared
        .block()
        .validate_output_merkle_cache()
        .expect("complete reveal outputs");
    assert_eq!(
        prepared.block().network_input_hashes().collect::<Vec<_>>(),
        [reveal_hash]
    );
    let (_, reveal_output) = prepared
        .block()
        .network_output_at(0)
        .expect("exact reveal Network output");
    let rejection = reveal_output
        .result
        .as_ref()
        .expect_err("conflicting sealed ballot must remain rejected");
    assert!(
        format!("{rejection:?}").contains("re-vote cannot change direction"),
        "unexpected rejection: {rejection:?}"
    );
    let finality = crate::state::publish_governance_fixture(&state, prepared, &keys);
    publications += 1;
    assert_eq!(executions, 3, "each signed candidate executes exactly once");
    assert_eq!(
        publications, 3,
        "each original carrier publishes exactly once"
    );
    assert_eq!(state.committed_height(), 3);
    assert_eq!(finality.height, 3);
    assert!(
        state.has_committed_entrypoint(reveal_hash),
        "the exact rejected sealed carrier must be replay protected"
    );
    assert!(
        state.has_committed_entrypoint(transaction.hash_as_entrypoint()),
        "the rejected reveal's enclosed signed intent must be replay protected"
    );
    // Escrow should now hold 16 (20 - 20% slash), slash receiver 4.
    let view = state.view();
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id);
    let slash_asset_id = AssetId::new(def_id.clone(), slash_id);
    let lock = view
        .world()
        .governance_locks()
        .get(&rid)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock present after slash");
    assert_eq!(lock.amount, Quantity::from(16_u64));
    assert_eq!(lock.slashed, Quantity::from(4_u64));
    let escrow_balance = view
        .world()
        .asset(&escrow_asset_id)
        .expect("escrow asset exists")
        .as_ref()
        .clone();
    let slash_balance = view
        .world()
        .asset(&slash_asset_id)
        .expect("slash receiver asset exists")
        .as_ref()
        .clone();
    assert_eq!(escrow_balance.clone(), Quantity::from(16_u64));
    assert_eq!(slash_balance.clone(), Quantity::from(4_u64));
    assert_eq!(
        view.world()
            .asset(&AssetId::new(def_id.clone(), alice.clone()))
            .expect("voter retains the unbonded balance")
            .as_ref(),
        &Quantity::from(980_u64),
        "voter 980 + escrow 16 + slash 4 conserve the original 1,000 units"
    );
    drop(view);
    let header4 = BlockHeader::new(nonzero!(4_u64), None, None, 0, 0);
    let mut sblock4 = state.block(header4);
    let mut stx4 = sblock4.transaction();
    let unresolved_revote = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: rid.clone(),
        direction: 0,
        owner: ALICE_ID.clone(),
        amount: 20_u64.into(),
        duration_blocks: 200,
    }
    .execute(&ALICE_ID, &mut stx4)
    .expect_err("a re-vote must not overwrite unresolved slash accounting");
    assert!(
        unresolved_revote
            .to_string()
            .contains("re-vote requires prior restitution")
    );
    let retained = stx4
        .world
        .governance_locks()
        .get(&rid)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("rejected re-vote retains the slashed lock");
    assert_eq!(retained.amount, Quantity::from(16_u64));
    assert_eq!(retained.slashed, Quantity::from(4_u64));
    assert_eq!(
        stx4.world
            .asset(&escrow_asset_id)
            .expect("escrow remains after rejected re-vote")
            .as_ref()
            .clone(),
        Quantity::from(16_u64)
    );
    assert_eq!(
        stx4.world
            .asset(&slash_asset_id)
            .expect("slash receiver remains after rejected re-vote")
            .as_ref()
            .clone(),
        Quantity::from(4_u64)
    );
}
#[test]
fn restitution_restores_slashed_balance() {
    // Direct retained-custody movements share the execution witness recorder.
    let _witness_guard = iroha_core::sumeragi::witness::exec_witness_guard();
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let (escrow_id, _) = gen_account_in("wonderland");
    let (slash_id, _) = gen_account_in("wonderland");
    let mut state = governance_state_with_accounts(def_id.clone(), &escrow_id, &slash_id);
    let alice = ALICE_ID.clone();
    let mut gov_cfg = state.gov.clone();
    // The retained position originally bonded 100 units before its 40-unit slash.
    gov_cfg.min_bond_amount = 100_u64.into();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = slash_id.clone();
    state.set_gov(gov_cfg);
    let rid = "rid-restitute".to_string();
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id.clone());
    let slash_asset_id = AssetId::new(def_id.clone(), slash_id.clone());
    // Pre-seed a lock with a recorded slash (amount=60 active, 40 slashed) and matching balances.
    seed_slash_snapshot(&mut state, &rid, &escrow_asset_id, &slash_asset_id);
    {
        let header = BlockHeader::new(nonzero!(3_u64), None, None, 0, 0);
        let mut sblock = state.block(header);
        let mut stx = sblock.transaction();
        // Grant restitution permission to ALICE.
        let perm: Permission = CanRestituteGovernanceLock {
            referendum_id: rid.clone(),
        }
        .into();
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("grant restitution permission");
        iroha_data_model::isi::governance::RestituteGovernanceLock {
            referendum_id: rid.clone(),
            owner: ALICE_ID.clone(),
            amount: 30_u64.into(),
            reason: "appeal_upheld".to_string(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("restitution should succeed");
        let events = stx.world.take_external_events();
        assert!(events.iter().any(|ev| {
            matches!(
                ev.as_data_event(),
                Some(iroha_data_model::events::data::DataEvent::Governance(
                    iroha_data_model::events::data::governance::GovernanceEvent::LockRestituted(payload)
                )) if payload.amount == Quantity::from(30_u64)
                    && payload.reason == GovernanceSlashReason::Restitution
                    && payload.note == "appeal_upheld"
            )
        }));
        stx.apply();
        sblock
            .commit_world_overlay_for_testing()
            .expect("commit restitution fixture");
    }
    let view = state.view();
    let lock = view
        .world()
        .governance_locks()
        .get(&rid)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock present after restitution");
    assert_eq!(lock.amount, Quantity::from(90_u64));
    assert_eq!(lock.slashed, Quantity::from(10_u64));
    let escrow_balance = view
        .world()
        .asset(&escrow_asset_id)
        .expect("escrow asset exists")
        .as_ref()
        .clone();
    let slash_balance = view
        .world()
        .asset(&slash_asset_id)
        .expect("slash receiver asset exists")
        .as_ref()
        .clone();
    assert_eq!(escrow_balance.clone(), Quantity::from(90_u64));
    assert_eq!(slash_balance.clone(), Quantity::from(10_u64));
}
#[test]
fn restitution_preflight_leaves_custody_untouched_when_slash_ledger_is_missing() {
    let def_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    );
    let (escrow_id, _) = gen_account_in("wonderland");
    let (slash_id, _) = gen_account_in("wonderland");
    let mut state = governance_state_with_accounts(def_id.clone(), &escrow_id, &slash_id);
    let mut gov_cfg = state.gov.clone();
    // The retained position originally bonded 100 units before its 40-unit slash.
    gov_cfg.min_bond_amount = 100_u64.into();
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = slash_id.clone();
    state.set_gov(gov_cfg);
    let referendum_id = "restitution-missing-ledger";
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id);
    let slash_asset_id = AssetId::new(def_id, slash_id);
    seed_slash_snapshot(&mut state, referendum_id, &escrow_asset_id, &slash_asset_id);
    {
        let header = BlockHeader::new(nonzero!(2_u64), None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_transaction = block.transaction();
        state_transaction
            .world
            .governance_slashes_mut()
            .remove(referendum_id.to_owned());
        Grant::account_permission(
            Permission::from(CanRestituteGovernanceLock {
                referendum_id: referendum_id.to_owned(),
            }),
            ALICE_ID.clone(),
        )
        .execute(&ALICE_ID, &mut state_transaction)
        .expect("grant restitution permission");
        state_transaction.apply();
        block
            .commit_empty_block_for_testing()
            .expect("commit restitution fixture block");
    }
    let header = BlockHeader::new(nonzero!(3_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut state_transaction = block.transaction();
    let error = iroha_data_model::isi::governance::RestituteGovernanceLock {
        referendum_id: referendum_id.to_owned(),
        owner: ALICE_ID.clone(),
        amount: 30_u64.into(),
        reason: "missing_ledger".to_owned(),
    }
    .execute(&ALICE_ID, &mut state_transaction)
    .expect_err("missing slash ledger must reject restitution");
    assert!(error.to_string().contains("slash ledger missing"));
    // Deliberately apply the errored overlay to prove the helper itself did not
    // stage any custody or lock mutation before its ledger preflight failed.
    state_transaction.apply();
    block
        .commit_empty_block_for_testing()
        .expect("commit restitution fixture block");
    let view = state.view();
    let lock = view
        .world()
        .governance_locks()
        .get(referendum_id)
        .and_then(|locks| locks.locks.get(&ALICE_ID))
        .expect("rejected restitution retains the lock");
    assert_eq!(lock.amount, Quantity::from(60_u64));
    assert_eq!(lock.slashed, Quantity::from(40_u64));
    assert_eq!(
        view.world()
            .asset(&escrow_asset_id)
            .expect("escrow asset")
            .as_ref(),
        &Quantity::from(60_u64)
    );
    assert_eq!(
        view.world()
            .asset(&slash_asset_id)
            .expect("slash receiver asset")
            .as_ref(),
        &Quantity::from(40_u64)
    );
}
#[test]
fn slash_and_restitution_use_stored_custody_after_governance_config_change() {
    // Direct retained-custody movements share the execution witness recorder.
    let _witness_guard = iroha_core::sumeragi::witness::exec_witness_guard();
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain");
    let old_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "old_xor".parse().expect("old asset name"),
    );
    let live_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "live_xor".parse().expect("live asset name"),
    );
    let alice = ALICE_ID.clone();
    let (old_escrow, _) = gen_account_in("wonderland");
    let (old_receiver, _) = gen_account_in("wonderland");
    let (live_escrow, _) = gen_account_in("wonderland");
    let (live_receiver, _) = gen_account_in("wonderland");
    let old_escrow_asset_id = AssetId::new(old_definition_id.clone(), old_escrow.clone());
    let old_receiver_asset_id = AssetId::new(old_definition_id.clone(), old_receiver.clone());
    let live_escrow_asset_id = AssetId::new(live_definition_id.clone(), live_escrow.clone());
    let live_receiver_asset_id = AssetId::new(live_definition_id.clone(), live_receiver.clone());
    let world = World::with_assets(
        [Domain::new(domain_id).build(&alice)],
        [
            iroha_data_model::account::Account::new(alice.clone()).build(&alice),
            iroha_data_model::account::Account::new(old_escrow.clone()).build(&alice),
            iroha_data_model::account::Account::new(old_receiver.clone()).build(&alice),
            iroha_data_model::account::Account::new(live_escrow.clone()).build(&alice),
            iroha_data_model::account::Account::new(live_receiver.clone()).build(&alice),
        ],
        [
            AssetDefinition::numeric(
                old_definition_id.clone(),
                "old_xor".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&alice),
            AssetDefinition::numeric(
                live_definition_id.clone(),
                "live_xor".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&alice),
        ],
        [
            Asset::new(old_escrow_asset_id.clone(), Quantity::from(10_u64)),
            Asset::new(old_receiver_asset_id.clone(), Quantity::from(5_u64)),
            Asset::new(live_escrow_asset_id.clone(), Quantity::from(11_u64)),
            Asset::new(live_receiver_asset_id.clone(), Quantity::from(13_u64)),
        ],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut live_governance = state.gov.clone();
    live_governance.voting_asset_id = live_definition_id;
    live_governance.bond_escrow_account = live_escrow;
    live_governance.slash_receiver_account = live_receiver;
    state.set_gov(live_governance);
    let referendum_id = "stored-custody-slash-restitution";
    let stored_custody = iroha_core::state::GovernanceLockCustody {
        escrowed: true,
        asset_definition_id: old_definition_id,
        bond_escrow_account: old_escrow,
        slash_receiver_account: old_receiver,
    };
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    iroha_core::query::standalone_plain_test_fixture::fund_voter(
        &mut tx,
        &iroha_test_samples::ALICE_ID,
        1_000_000_u64.into(),
        0,
    );
    let mut frozen_governance = tx.gov.clone();
    // The old custody position bonded ten; the newer live minimum remains unchanged.
    frozen_governance.min_bond_amount = 10_u64.into();
    frozen_governance.voting_asset_id = stored_custody.asset_definition_id.clone();
    frozen_governance.bond_escrow_account = stored_custody.bond_escrow_account.clone();
    frozen_governance.slash_receiver_account = stored_custody.slash_receiver_account.clone();
    tx.world.governance_referenda_mut().insert(
        referendum_id.to_owned(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 99,
            status: iroha_core::state::GovernanceReferendumStatus::Open,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
            plain_context: iroha_core::query::standalone_plain_test_fixture::context(
                &frozen_governance,
                0,
            ),
            plain_result: iroha_data_model::governance::conviction::PlainVotingResultV1::Pending,
        },
    );
    for permission in [
        Permission::from(CanSlashGovernanceLock {
            referendum_id: referendum_id.to_owned(),
        }),
        Permission::from(CanRestituteGovernanceLock {
            referendum_id: referendum_id.to_owned(),
        }),
    ] {
        Grant::account_permission(permission, alice.clone())
            .execute(&alice, &mut tx)
            .expect("grant governance custody permission");
    }
    let mut locks = iroha_core::state::GovernanceLocksForReferendum::default();
    locks.locks.insert(
        alice.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: alice.clone(),
            amount: Quantity::from(10_u64),
            slashed: Quantity::zero(),
            expiry_height: 100,
            direction: 0,
            duration_blocks: 100,
            custody: stored_custody.clone(),
        },
    );
    tx.world
        .governance_locks_mut()
        .insert(referendum_id.to_owned(), locks);
    iroha_data_model::isi::governance::SlashGovernanceLock {
        referendum_id: referendum_id.to_owned(),
        owner: alice.clone(),
        amount: Quantity::from(4_u64),
        reason: "stored custody regression".to_owned(),
    }
    .execute(&alice, &mut tx)
    .expect("slash must use the lock's stored custody");
    let lock_after_slash = tx
        .world
        .governance_locks()
        .get(referendum_id)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock after slash");
    assert_eq!(lock_after_slash.amount, Quantity::from(6_u64));
    assert_eq!(lock_after_slash.slashed, Quantity::from(4_u64));
    assert_eq!(lock_after_slash.custody, stored_custody);
    assert_eq!(
        tx.world
            .asset(&old_escrow_asset_id)
            .expect("stored escrow asset after slash")
            .as_ref()
            .clone(),
        Quantity::from(6_u64)
    );
    assert_eq!(
        tx.world
            .asset(&old_receiver_asset_id)
            .expect("stored slash receiver asset after slash")
            .as_ref()
            .clone(),
        Quantity::from(9_u64)
    );
    assert_eq!(
        tx.world
            .asset(&live_escrow_asset_id)
            .expect("live escrow asset after slash")
            .as_ref()
            .clone(),
        Quantity::from(11_u64)
    );
    assert_eq!(
        tx.world
            .asset(&live_receiver_asset_id)
            .expect("live slash receiver asset after slash")
            .as_ref()
            .clone(),
        Quantity::from(13_u64)
    );
    let ledger_after_slash = tx
        .world
        .governance_slashes()
        .get(referendum_id)
        .and_then(|ledger| ledger.slashes.get(&alice))
        .expect("slash ledger entry");
    assert_eq!(ledger_after_slash.total_slashed, Quantity::from(4_u64));
    assert_eq!(ledger_after_slash.total_restituted, Quantity::zero());
    assert_eq!(
        ledger_after_slash.last_reason,
        GovernanceSlashReason::Manual
    );
    assert_eq!(ledger_after_slash.last_height, 1);
    iroha_data_model::isi::governance::RestituteGovernanceLock {
        referendum_id: referendum_id.to_owned(),
        owner: alice.clone(),
        amount: Quantity::from(4_u64),
        reason: "stored custody appeal".to_owned(),
    }
    .execute(&alice, &mut tx)
    .expect("restitution must use the lock's stored custody");
    let lock_after_restitution = tx
        .world
        .governance_locks()
        .get(referendum_id)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock after restitution");
    assert_eq!(lock_after_restitution.amount, Quantity::from(10_u64));
    assert_eq!(lock_after_restitution.slashed, Quantity::zero());
    assert_eq!(lock_after_restitution.custody, stored_custody);
    assert_eq!(
        tx.world
            .asset(&old_escrow_asset_id)
            .expect("stored escrow asset after restitution")
            .as_ref()
            .clone(),
        Quantity::from(10_u64)
    );
    assert_eq!(
        tx.world
            .asset(&old_receiver_asset_id)
            .expect("stored slash receiver asset after restitution")
            .as_ref()
            .clone(),
        Quantity::from(5_u64)
    );
    assert_eq!(
        tx.world
            .asset(&live_escrow_asset_id)
            .expect("live escrow asset after restitution")
            .as_ref()
            .clone(),
        Quantity::from(11_u64)
    );
    assert_eq!(
        tx.world
            .asset(&live_receiver_asset_id)
            .expect("live slash receiver asset after restitution")
            .as_ref()
            .clone(),
        Quantity::from(13_u64)
    );
    let ledger_after_restitution = tx
        .world
        .governance_slashes()
        .get(referendum_id)
        .and_then(|ledger| ledger.slashes.get(&alice))
        .expect("slash ledger after restitution");
    assert_eq!(
        ledger_after_restitution.total_slashed,
        Quantity::from(4_u64)
    );
    assert_eq!(
        ledger_after_restitution.total_restituted,
        Quantity::from(4_u64)
    );
    assert_eq!(
        ledger_after_restitution.last_reason,
        GovernanceSlashReason::Restitution
    );
    assert_eq!(ledger_after_restitution.last_height, 1);
}
