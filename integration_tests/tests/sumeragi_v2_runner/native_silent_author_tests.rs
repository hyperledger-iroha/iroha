// Real process-outage coverage. No packet loss is claimed: the author exits
// before the only submitted input exists, and stays offline through finality.

fn first_native_author(seed: [u8; 32], incarnation: Hash) -> usize {
    use iroha_model_base::topology::{DataSpaceId, LaneId};
    let seed = Hash::new_from_chunks(&[
        b"iroha:lane-consensus:leader-seed:v1\0",
        &seed,
        &LaneId::SINGLE.as_u32().to_be_bytes(),
        &DataSpaceId::UNIVERSAL.as_u64().to_be_bytes(),
        incarnation.as_ref(),
        &1_u64.to_be_bytes(),
    ]);
    seed.as_ref().iter().fold(0_usize, |offset, byte| {
        (offset * 256 + usize::from(*byte)) % VALIDATOR_COUNT
    })
}

fn verify_silent_native_decision(
    frozen: &iroha_data_model::block::lane_consensus::FrozenLaneConsensusContextV1,
    opening: &iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
    group: &iroha_data_model::block::lane_input::LaneDecisionGroupV1,
    silent: &PeerId,
) -> Result<()> {
    use iroha_core::state::{native_lane_instance_for_testing, native_lane_manifest_for_testing};
    use iroha_crypto::Signature;
    group.validate_structure().map_err(|error| eyre!(error))?;
    ensure!(
        group.decisions.len() == 1 && group.payload.descriptor.slots.len() == 1,
        "first single-lane input must own exactly one Native Decision"
    );
    let slot = &group.payload.descriptor.slots[0];
    let decision = &group.decisions[0];
    decision.validate_shape(frozen)?;
    ensure!(
        frozen.committee.len() == VALIDATOR_COUNT
            && frozen.next_lane_height == 1
            && frozen.predecessor_height == 0
            && frozen.predecessor_hash.is_none()
            && frozen.predecessor_applied_global_height == 0
            && slot.instance_id
                == native_lane_instance_for_testing(frozen.clone(), opening)
                    .map_err(|error| eyre!(error))?,
        "Decision is not the exact first slot under its real finalized opening"
    );
    // These helpers only derive unsigned evidence. Both calls use the actual
    // finalized opening and retained input; they never create a State capability.
    let initial = native_lane_manifest_for_testing(frozen.clone(), opening, &group.payload, 0)
        .map_err(|error| eyre!(error))?;
    ensure!(
        frozen.committee.get(initial.value.origin_producer as usize) == Some(silent),
        "the physically stopped validator was not the actual Native view-zero author"
    );
    ensure!(
        initial.value.origin_producer as usize
            == first_native_author(frozen.leader_seed, frozen.lane_incarnation),
        "pre-outage author prediction differs from the production Native derivation"
    );
    let value = decision.manifest.value;
    ensure!(
        (1..=VALIDATOR_COUNT as u64).contains(&value.origin_view)
            && (value.origin_view..=VALIDATOR_COUNT as u64)
                .contains(&decision.commit_qc.statement.round.voting_view)
            && frozen.committee.get(value.origin_producer as usize) != Some(silent),
        "the finite input was not created and committed by a later Native author within one rotation"
    );
    let expected = native_lane_manifest_for_testing(
        frozen.clone(),
        opening,
        &group.payload,
        value.origin_view,
    )
    .map_err(|error| eyre!(error))?;
    ensure!(
        expected == decision.manifest,
        "Native origin, original input or regenerated mandatory RS16 codeword changed"
    );
    ensure!(
        decision.commit_qc.shares.len() == 3,
        "four-validator Native Commit requires exactly three shares"
    );
    let preimage = decision.commit_qc.statement.signature_preimage()?;
    for share in &decision.commit_qc.shares {
        let signer = frozen
            .committee
            .get(share.signer as usize)
            .ok_or_else(|| eyre!("Native signer is outside the frozen committee"))?;
        ensure!(
            signer != silent,
            "offline author somehow supplied a Native Commit vote"
        );
        Signature::try_from_bytes(&share.signature)?.verify(signer.public_key(), &preimage)?;
    }
    Ok(())
}

/// The first Native author exits before a single finite input is submitted.
/// Three real validators must create a later-view value, execute it, and retain
/// identical authenticated results when the original author restarts.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_silent_initial_author_finalizes_one_finite_input() -> Result<()> {
    use iroha_core::{
        merge::{merge_lane_config_hash, merge_lane_consensus_catalog_hash},
        sumeragi::{signed_genesis_validator_pops, validate_signed_genesis_v2_authority},
        torii_proxy::decode_and_validate_lane_admitted_input_v1,
    };
    use iroha_data_model::{
        block::lane_consensus::FrozenLaneConsensusContextV1,
        bridge::BridgeFinalityVerifier,
        isi::staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        nexus::LaneCatalog,
        transaction::{FeePaymentIntent, signed::TransactionEntrypoint},
    };
    use iroha_model_base::topology::{DataSpaceId, LaneId};
    use norito::codec::Encode;
    const MAX_CARRIERS: u64 = 32;
    const OUTAGE_BOUND: Duration = Duration::from_secs(180);
    init_instruction_registry();
    let context = stringify!(native_silent_initial_author_finalizes_one_finite_input);
    let builder = four_validator_npos_builder()
        .with_base_seed(context)
        .with_block_cadence(RESTART_BLOCK_CADENCE)
        .with_config_layer(|layer| {
            // The launcher strips environment overrides. Keep authenticated
            // Native rejection diagnostics in the retained outage evidence.
            layer.write(["logger", "filter"], "iroha_core::sumeragi=debug,irohad=debug");
        })
        .with_sync_timeout(OUTAGE_BOUND);
    let network = sandbox::start_network_async_or_skip(builder, context)
        .await?
        .ok_or_else(|| {
            eyre!("Native process-outage regression requires a real four-validator network")
        })?;
    let result: Result<()> = async {
        let peers = network.peers().to_vec();
        ensure!(
            peers.len() == VALIDATOR_COUNT
                && network.topology_entries().len() == VALIDATOR_COUNT
                && peers.iter().all(NetworkPeer::is_running),
            "fresh Native outage fixture requires exactly four running voters"
        );
        let genesis = network.genesis();
        let network_id = network.network_id();
        ensure!(
            genesis.0.execution_context().is_none(),
            "fresh genesis must not carry pre-opened Native slots"
        );
        let voters = signed_genesis_validator_pops(&genesis)?;
        let committee = voters.keys().cloned().collect::<Vec<_>>();
        ensure!(
            committee.len() == VALIDATOR_COUNT
                && committee.iter().cloned().collect::<BTreeSet<_>>()
                    == peers.iter().map(NetworkPeer::id).collect(),
            "signed genesis differs from the exact four process identities"
        );
        let mut registered = BTreeMap::new();
        let mut signed_npos = None;
        let mut activated = BTreeSet::new();
        for transaction in genesis.0.external_transactions() {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                return Err(eyre!("fixture genesis must use explicit instructions"));
            };
            for instruction in instructions {
                if let Some(set) = instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::SetParameter>()
                    && let iroha_data_model::parameter::Parameter::Custom(custom) = set.inner()
                    && custom.id() == &SumeragiNposParameters::parameter_id()
                {
                    let parameters = SumeragiNposParameters::from_custom_parameter(custom)
                        .ok_or_else(|| eyre!("invalid signed genesis NPoS snapshot"))?;
                    ensure!(
                        signed_npos.replace(parameters).is_none(),
                        "duplicate signed NPoS snapshot"
                    );
                }
                if let Some(register) = instruction
                    .as_any()
                    .downcast_ref::<RegisterPublicLaneValidator>()
                {
                    ensure!(
                        register.lane_id == LaneId::SINGLE && !register.initial_stake.is_zero(),
                        "fixture lane authority differs from funded primary-lane genesis"
                    );
                    ensure!(
                        registered
                            .insert(register.validator.clone(), register.peer_id.clone())
                            .is_none(),
                        "duplicate genesis lane validator"
                    );
                }
                if let Some(activate) = instruction
                    .as_any()
                    .downcast_ref::<ActivatePublicLaneValidator>()
                {
                    ensure!(
                        activate.lane_id == LaneId::SINGLE
                            && activated.insert(activate.validator.clone()),
                        "foreign or repeated lane activation"
                    );
                }
            }
        }
        ensure!(
            registered.keys().cloned().collect::<BTreeSet<_>>() == activated
                && registered.len() == VALIDATOR_COUNT
                && registered.values().cloned().collect::<BTreeSet<_>>()
                    == committee.iter().cloned().collect(),
            "Native committee must be the exact active four-validator pool signed into genesis"
        );
        let signed_npos =
            signed_npos.ok_or_else(|| eyre!("missing signed genesis NPoS snapshot"))?;
        wait_for_normal_statuses(&peers, 1, STATUS_TIMEOUT).await?;
        let genesis_proof = fetch_bridge_finality_proof(&peers[0], 1).await?;
        let genesis_context = &genesis_proof.finality_artifact.height_context;
        validate_signed_genesis_v2_authority(
            &genesis,
            genesis_context,
            &genesis_proof.finality_artifact.validator_set_pops,
        )?;
        ensure!(
            genesis_proof.block_header.hash() == genesis.0.hash()
                && genesis_context.mode == ConsensusMode::Npos
                && genesis_context.roster.iter().all(|entry| entry.power == 1)
                && genesis_context.quorum.min_signers == 3
                && genesis_context.quorum.total_power == 4
                && genesis_context.leader_seed == signed_npos.epoch_seed()
                && genesis_context.epoch_end_height == signed_npos.epoch_length_blocks().get()
                && genesis_context.epoch_end_height > MAX_CARRIERS,
            "fresh signed genesis identity, epoch seed or bounded stable epoch changed"
        );
        let mut finality = BridgeFinalityVerifier::with_context(network_id, genesis_context.id());
        finality.verify(&genesis_proof)?;
        let catalog = LaneCatalog::default();
        let lane = &catalog.lanes()[0];
        ensure!(
            lane.id == LaneId::SINGLE && lane.dataspace_id == DataSpaceId::UNIVERSAL,
            "fixture must use the default single public route"
        );
        let static_bytes = (
            merge_lane_consensus_catalog_hash(&catalog),
            lane.id,
            merge_lane_config_hash(lane),
        )
            .encode();
        let incarnation =
            Hash::new_from_chunks(&[b"iroha:nexus:lane-incarnation:static:v2\0", &static_bytes]);
        let silent_id =
            committee[first_native_author(genesis_context.leader_seed, incarnation)].clone();
        let silent = peers
            .iter()
            .find(|peer| peer.id() == silent_id)
            .ok_or_else(|| eyre!("predicted Native author has no process"))?
            .clone();
        let account = fixture_account(0xD1)?;
        assert_accounts_absent(&peers, &[account.clone()]).await?;
        let initial = normal_statuses(&peers).await?;
        ensure!(
            initial
                .iter()
                .all(|status| (1..MAX_CARRIERS).contains(&status.blocks)),
            "pre-input control carriers exceeded the bounded fresh epoch: {initial:?}"
        );
        let config_layers = network.config_layers().collect::<Vec<_>>();
        silent.shutdown().await;
        ensure!(
            !silent.is_running(),
            "initial Native author must exit before submission"
        );
        let survivors = peers
            .iter()
            .filter(|peer| peer.is_running())
            .cloned()
            .collect::<Vec<_>>();
        ensure!(survivors.len() == 3, "exactly three processes must survive");
        let client = survivors[0].client();
        let submitted_account = account.clone();
        // submit waits for Applied. This is the sole submission, with no producer
        // providing fresh work to disguise a stranded final transaction.
        let transaction_hash = tokio::time::timeout(
            OUTAGE_BOUND,
            run_blocking_sdk(move || {
                client.submit(
                    Register::account(Account::new(submitted_account)),
                    FeePaymentIntent::authority(Vec::new(), None),
                )
            }),
        )
        .await
        .wrap_err("finite input exceeded the bounded Native author-outage interval")???;
        ensure!(
            !silent.is_running(),
            "the original author restarted before Apply"
        );
        wait_for_accounts_visible(&survivors, &[account.clone()], ACCOUNT_VISIBILITY_TIMEOUT)
            .await?;
        let statuses = normal_statuses(&survivors).await?;
        let committed = statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or(0);
        ensure!(
            (3..=MAX_CARRIERS).contains(&committed),
            "missing or unbounded admission/execution carriers"
        );
        let mut retained = BTreeMap::new();
        let mut first_admission = None;
        let mut executed = None;
        for height in 2..=committed {
            let proof = fetch_bridge_finality_proof(&survivors[0], height).await?;
            finality.verify(&proof)?;
            let artifact = &proof.finality_artifact;
            ensure!(
                artifact.height_context.epoch == genesis_context.epoch
                    && artifact.height_context.leader_seed == genesis_context.leader_seed
                    && artifact.height_context.roster == genesis_context.roster
                    && artifact.validator_set_pops
                        == genesis_proof.finality_artifact.validator_set_pops
                    && artifact.commit_qc.signers.len() == 3,
                "outage must preserve the original exact committee and signed epoch"
            );
            let block = committed_block_at_height(&survivors[0], height).await?;
            let wire = block.encode_wire()?;
            let execution = artifact.commit_qc.execution_commitment;
            ensure!(
                block.header() == proof.block_header
                    && artifact.subject.payload_hash == block.canonical_proposal_wire_hash()?
                    && execution.executed_block_wire_len == wire.len() as u64
                    && execution.executed_block_wire_hash == Hash::new(&wire),
                "global finality does not bind this complete executed Native carrier"
            );
            block.validate_output_merkle_cache()?;
            ensure!(
                block.external_entrypoints_slice().is_empty(),
                "post-genesis economic input bypassed Native execution"
            );
            if let Some(bundle) = block.execution_context() {
                for (index, bytes) in bundle.queue_plan_admissions.iter().enumerate() {
                    let input = decode_and_validate_lane_admitted_input_v1(&network_id, bytes)
                        .map_err(|error| eyre!(error))?;
                    ensure!(
                        matches!(&input.input().entrypoint,
                        TransactionEntrypoint::External(tx) if tx.hash() == transaction_hash),
                        "unexpected economic input invalidates the fresh first-slot fixture"
                    );
                    if let Some((_, _, _, first)) = &first_admission {
                        ensure!(
                            first == input.input(),
                            "repeated admission changed the original complete input"
                        );
                    }
                    first_admission.get_or_insert((
                        height,
                        index,
                        block.hash(),
                        input.input().clone(),
                    ));
                }
                if let Some(batch) = &bundle.native_lane_decisions {
                    batch.validate_structure().map_err(|error| eyre!(error))?;
                    ensure!(
                        executed.is_none() && batch.groups.len() == 1,
                        "the finite input must execute once in one Native group"
                    );
                    let group = &batch.groups[0];
                    let (admission_height, admission_index, admission_hash, input) =
                        first_admission.as_ref().ok_or_else(|| {
                            eyre!("Native execution precedes first complete admission")
                        })?;
                    ensure!(
                        group.payload.input == *input
                            && group.payload.descriptor.admission_priority.carrier_height
                                == *admission_height
                            && group.payload.descriptor.admission_priority.admission_index as usize
                                == *admission_index
                            && group.payload.descriptor.admission_carrier_hash == *admission_hash,
                        "Native execution substituted its first authenticated admission"
                    );
                    let opening: &BridgeFinalityProof =
                        retained.get(admission_height).ok_or_else(|| {
                            eyre!("first opening finality was not retained before execution")
                        })?;
                    let authority = &opening.finality_artifact.height_context;
                    ensure!(
                        group.payload.descriptor.slots.len() == 1,
                        "unexpected multi-route input"
                    );
                    let slot = &group.payload.descriptor.slots[0];
                    ensure!(
                        slot.route.lane_id == lane.id
                            && slot.route.dataspace_id == lane.dataspace_id
                            && slot.lane_incarnation == incarnation
                            && slot.lane_height == 1,
                        "actual finalized route differs from the pre-outage author prediction"
                    );
                    let frozen = FrozenLaneConsensusContextV1 {
                        network_id,
                        protocol_version: authority.protocol_version,
                        opening_global_height: *admission_height,
                        opening_global_context_id: authority.id(),
                        admitted_binding_hash: input.certificate.binding.canonical_hash(),
                        admission_priority: group.payload.descriptor.admission_priority,
                        epoch: authority.epoch,
                        mode: authority.mode,
                        lane_id: lane.id,
                        dataspace_id: lane.dataspace_id,
                        lane_incarnation: incarnation,
                        next_lane_height: 1,
                        predecessor_height: 0,
                        predecessor_hash: None,
                        predecessor_applied_global_height: 0,
                        committee: committee.clone(),
                        validator_set_pops: voters.values().cloned().collect(),
                        nexus_amx_context_hash: authority.nexus_amx_context_hash,
                        execution_policy_hash: authority.execution_policy_hash,
                        da_layout: authority.da_layout,
                        leader_seed: authority.leader_seed,
                    };
                    verify_silent_native_decision(
                        &frozen,
                        &opening.finality_artifact,
                        group,
                        &silent_id,
                    )?;
                    let other = committee.iter().find(|peer| *peer != &silent_id).unwrap();
                    ensure!(
                        verify_silent_native_decision(
                            &frozen,
                            &opening.finality_artifact,
                            group,
                            other
                        )
                        .is_err(),
                        "a different stopped peer must not satisfy the silent-author proof"
                    );
                    let mut foreign = frozen.clone();
                    foreign.lane_incarnation = Hash::new(b"foreign Native incarnation");
                    ensure!(
                        verify_silent_native_decision(
                            &foreign,
                            &opening.finality_artifact,
                            group,
                            &silent_id
                        )
                        .is_err(),
                        "a foreign opening must not satisfy the silent-author proof"
                    );
                    let (_, output) = block.network_output_at(0).ok_or_else(|| {
                        eyre!("Native execution omitted its actual Network result")
                    })?;
                    ensure!(
                        output.result.0.is_ok() && block.network_entrypoints().count() == 1,
                        "the finite input must have one successful economic execution"
                    );
                    executed = Some((height, wire));
                }
            }
            retained.insert(height, proof);
        }
        let (height, executed_wire) = executed
            .ok_or_else(|| eyre!("Applied account lacks a real Native Decision carrier"))?;
        assert_restart_finality(&survivors, &peers, &network_id, height).await?;
        silent
            .start_checked(config_layers.iter().cloned(), None)
            .await?;
        network
            .ensure_blocks_with(|status| status.total >= height)
            .await?;
        wait_for_accounts_visible(&peers, &[account], ACCOUNT_VISIBILITY_TIMEOUT).await?;
        for peer in &peers {
            ensure!(
                committed_block_at_height(peer, height)
                    .await?
                    .encode_wire()?
                    == executed_wire,
                "restarted and surviving validators must retain identical complete execution bytes"
            );
        }
        assert_restart_finality(&peers, &peers, &network_id, height).await?;
        Ok(())
    }
    .await;
    network.shutdown_and_release().await;
    result
}
