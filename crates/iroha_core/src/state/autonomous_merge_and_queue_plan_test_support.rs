#[expect(
    clippy::too_many_lines,
    reason = "the fixture assembles one complete availability-certified autonomous source"
)]
fn autonomous_merge_source_for_queue_plan_admission_test(
    state: &State,
    binding: &crate::torii_proxy::QueuePlanAdmissionBindingV2,
    entrypoint: TransactionEntrypoint,
    routing_plan: crate::queue::RoutingPlan,
    activation_validator_keypairs: &[KeyPair],
) -> MergeExecutionSource {
    let coordinator = binding
        .admission_context
        .route_incarnations
        .first()
        .expect("fixture binding has a coordinator");
    let proposal_height = binding.admission_context.proposal_height;
    // This fixture has one active execution lane, so production merge
    // validation binds autonomous certification to the global commit
    // topology rather than the QueuePlan admission committee.
    let mut validator_set = state.commit_topology_snapshot();
    validator_set.sort();
    validator_set.dedup();
    assert!(
        !validator_set.is_empty(),
        "fixture activation committee must not be empty"
    );
    let validator_count =
        u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
    let min_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
        validator_set.len(),
    ))
    .expect("fixture quorum fits u32");
    let lane_block_height = 1;
    let lane_block_view = 0;
    let entrypoint_hash = Hash::from(entrypoint.hash());
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: coordinator.leg.route.lane_id,
        dataspace_id: coordinator.leg.route.dataspace_id,
        lane_incarnation: coordinator.lane_incarnation,
        proposal_height,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height,
        lane_block_view,
        subject_hash: Hash::new(b"queue-plan-pre-carrier-autonomous-subject"),
        payload_ownership_hash: Hash::new(b"queue-plan-pre-carrier-autonomous-ownership"),
        rbc_instance_hash: Hash::new(b"queue-plan-pre-carrier-autonomous-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![entrypoint_hash],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count,
        min_quorum,
        qc_mode_tag: "permissioned:queue-plan-pre-carrier-autonomous".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();

    let accepted =
        crate::tx::AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
    let reservation = crate::queue::LaneQueueReservationKeyV2 {
        version: crate::queue::LaneQueueReservationKeyV2::VERSION,
        signed_transaction_hash: accepted.hash(),
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: binding.canonical_hash(),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height,
        lane_block_height,
        lane_block_view,
        reservation_owner_hash: Hash::new(b"queue-plan-pre-carrier-autonomous-reservation-owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let producer =
        crate::lane_consensus::deterministic_lane_author(&validator_set, lane_block_height)
            .cloned()
            .expect("fixture activation committee has a deterministic lane author");
    let producer_keypair = activation_validator_keypairs
        .iter()
        .find(|keypair| keypair.public_key() == producer.public_key())
        .expect("fixture retains the deterministic producer key");
    let chain_id_hash = Hash::new(state.chain_id.clone().into_inner().as_bytes());
    let epoch = crate::sumeragi::epoch_for_height_from_world(&state.world.view(), proposal_height);
    let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
        chain_id_hash,
        epoch,
        proposal.clone(),
        vec![entrypoint.clone()],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        producer,
        producer_keypair.private_key(),
    )
    .expect("canonical autonomous QueuePlan fixture payload");

    let validator_pops = validator_set
        .iter()
        .map(|validator| {
            let keypair = activation_validator_keypairs
                .iter()
                .find(|keypair| keypair.public_key() == validator.public_key())
                .expect("fixture retains every lane validator key");
            iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                .expect("fixture lane validator PoP")
        })
        .collect::<Vec<_>>();
    let selected_keypairs = validator_set
        .iter()
        .take(usize::try_from(min_quorum).expect("fixture quorum fits usize"))
        .map(|validator| {
            activation_validator_keypairs
                .iter()
                .find(|keypair| keypair.public_key() == validator.public_key())
                .expect("fixture retains every selected lane validator key")
        })
        .collect::<Vec<_>>();
    let prepare_body = proposal.vote_body(CertPhase::Prepare);
    let availability_body = crate::lane_consensus::lane_payload_availability_body(
        &payload,
        &proposal,
        chain_id_hash,
        epoch,
    )
    .expect("fixture availability body");
    let prepare_votes = selected_keypairs
        .iter()
        .map(|keypair| {
            let availability_vote =
                crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                    availability_body.clone(),
                    PeerId::new(keypair.public_key().clone()),
                    validator_pops.clone(),
                    keypair.private_key(),
                )
                .expect("fixture availability vote");
            crate::lane_consensus::LaneBlockVoteV1 {
                body: prepare_body.clone(),
                signer: PeerId::new(keypair.public_key().clone()),
                bls_signature: Signature::try_new(
                    keypair.private_key(),
                    &prepare_body.signature_preimage(),
                )
                .expect("fixture prepare signature")
                .payload()
                .to_vec(),
                payload_availability_vote: Some(availability_vote),
            }
        })
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        prepare_body,
        validator_set.clone(),
        &prepare_votes,
    )
    .expect("fixture availability-certified PrepareQC");
    let commit_votes = selected_keypairs
        .iter()
        .map(|keypair| signed_lane_block_vote_for_state_test(&proposal, CertPhase::Commit, keypair))
        .collect::<Vec<_>>();
    let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Commit),
        validator_set,
        &commit_votes,
    )
    .expect("fixture CommitQC");
    let signer_pops = selected_keypairs
        .iter()
        .map(|keypair| {
            (
                keypair.public_key().clone(),
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("fixture selected signer PoP"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let certified = crate::kura::CertifiedLaneBlockArtifact::new(
        crate::lane_consensus::CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: prepare_qc.clone(),
            commit_qc,
        },
        signer_pops,
    );
    let autonomous = crate::kura::AutonomousLaneBlockArtifact {
        format: crate::kura::AutonomousLaneBlockArtifactFormat::Current,
        executable_payload: payload.clone(),
        availability_certificate: Some(
            crate::lane_consensus::DurableLanePayloadAvailabilityCertificateV1 {
                certificate: prepare_qc,
            },
        ),
        view_checkpoint: None,
        new_view_certificates: Vec::new(),
    };
    let bundle = crate::kura::AutonomousLaneMergeBundleV1 {
        version: crate::kura::AutonomousLaneMergeBundleV1::VERSION,
        autonomous,
        certified: certified.clone(),
    };
    let source_bundle = bundle
        .encode_framed()
        .expect("fixture autonomous bundle encoding");
    crate::kura::Kura::validate_autonomous_lane_merge_bundle(&bundle, chain_id_hash, epoch)
        .expect("fixture autonomous bundle validation");
    let (proposal_block_hash, proposal_view) =
        crate::kura::Kura::autonomous_lane_execution_anchor(&proposal, payload.payload_hash);
    let descriptor = &proposal.descriptor;
    let ownership = SumeragiLanePayloadOwnership {
        proposal_height,
        proposal_view,
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        lane_block_height,
        lane_block_view,
        subject_hash: descriptor.subject_hash,
        qc_mode_tag: descriptor.qc_mode_tag.clone(),
        accepted_candidate_indices: descriptor.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: descriptor.accepted_transaction_hashes.clone(),
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_descriptor_hash: Some(descriptor.descriptor_hash),
        lane_block_descriptor_validator_set: descriptor.validator_set.clone(),
        lane_block_descriptor_validator_count: descriptor.validator_count,
        lane_block_descriptor_min_quorum: descriptor.min_quorum,
        payload_ownership_hash: descriptor.payload_ownership_hash,
        rbc_instance_hash: descriptor.rbc_instance_hash,
    };
    let input =
        crate::kura::LaneBlockExecutionInputArtifact::new(crate::kura::RecoveredLaneBlockPayload {
            proposal: proposal.clone(),
            artifact: crate::kura::LaneBlockArtifact::new(proposal_block_hash, ownership),
            autonomous_chain_id_hash: Some(chain_id_hash),
            autonomous_epoch: Some(epoch),
            autonomous_payload_hash: Some(payload.payload_hash),
            entrypoints: vec![entrypoint],
            reservation_keys: payload.reservation_keys.clone(),
            routing_plans: payload.routing_plans.clone(),
            native_amx_receipts: payload.native_amx_receipts.clone(),
        });
    MergeExecutionSource {
        bundle_hash: merge_execution_source_bundle_hash(&source_bundle),
        source_bundle,
        origin_proposal: proposal,
        certified,
        input,
    }
}

fn seed_exact_queue_plan_admission_state_for_test(state: &State, certificate: &[u8]) {
    let admission = crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v2(
        &state.chain_id,
        certificate,
    )
    .expect("fixture QueuePlan admission certificate");
    let mut world = state.world.block();
    world.smart_contract_state.insert(
        State::queue_plan_admission_registry_marker_key(&admission.registry_key)
            .expect("fixture registry key"),
        State::queue_plan_admission_registry_marker_payload(&admission.registry_value)
            .expect("fixture registry value"),
    );
    State::stage_queue_plan_pending_obligation_in_storage(
        &mut world.smart_contract_state,
        &admission,
    )
    .expect("fixture pending QueuePlan obligation");
    world.commit();
}

fn seed_pending_queue_plan_binding_state_for_test(
    state: &State,
    binding: &crate::torii_proxy::QueuePlanAdmissionBindingV2,
) {
    state
        .install_queue_plan_pending_binding_for_test(binding)
        .expect("fixture pending QueuePlan binding");
}

fn queue_plan_pending_obligation_for_test(
    state: &State,
    certificate: &[u8],
) -> QueuePlanPendingObligationV1 {
    let admission = crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v2(
        &state.chain_id,
        certificate,
    )
    .expect("fixture QueuePlan admission certificate");
    State::queue_plan_pending_obligation_from_admission(&admission)
        .expect("fixture pending QueuePlan obligation")
}

fn clear_exact_queue_plan_admission_state_for_test(state: &State, certificate: &[u8]) {
    let admission = crate::torii_proxy::decode_and_validate_queue_plan_admission_certificate_v2(
        &state.chain_id,
        certificate,
    )
    .expect("fixture QueuePlan admission certificate");
    let binding = &admission.certificate.binding;
    let mut world = state.world.block();
    assert!(
        State::resolve_queue_plan_pending_obligation_in_storage(
            &mut world.smart_contract_state,
            binding.chain_id_digest,
            binding.entrypoint_hash.clone(),
        )
        .expect("resolve fixture pending QueuePlan obligation")
    );
    world.smart_contract_state.remove(
        State::queue_plan_admission_registry_marker_key(&admission.registry_key)
            .expect("fixture registry key"),
    );
    world.commit();
}

fn persist_merge_carrier_finality_chain_for_state_test(
    state: &State,
    parent: &SignedBlock,
    carrier: &SignedBlock,
    keypairs: &[KeyPair],
) {
    use iroha_data_model::block::consensus_v2::{
        BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
        ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
        QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
    };

    fn artifact_for_block(
        state: &State,
        block: &SignedBlock,
        parent: Option<&V2FinalityArtifact>,
        keypairs: &[KeyPair],
    ) -> V2FinalityArtifact {
        assert!(!keypairs.is_empty(), "finality fixture requires validators");
        let mut keypairs = keypairs.iter().collect::<Vec<_>>();
        keypairs.sort_by_key(|keypair| PeerId::new(keypair.public_key().clone()));
        let roster = keypairs
            .iter()
            .map(|keypair| ValidatorPower {
                validator: PeerId::new(keypair.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let height = block.header().height().get();
        assert_eq!(
            parent.map_or(1, |artifact| artifact.height.saturating_add(1)),
            height,
            "fixture finality must form one contiguous chain",
        );
        let context = HeightContext {
            chain_id: state.chain_id.clone(),
            protocol_version: PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: parent.map(|artifact| artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid finality quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"state merge finality nexus context"),
            execution_policy_hash: Hash::new(b"state merge finality execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4_096,
                max_chunk_count: 8,
            },
            leader_seed: [0x42; 32],
        };
        let executed_block_wire = block.encode_wire().expect("canonical executed block wire");
        let height_bytes = height.to_le_bytes();
        let mut execution_commitment = ExecutionCommitment::new_without_merge_carrier(
            Hash::new_from_chunks(&[
                b"state merge finality parent state".as_slice(),
                height_bytes.as_slice(),
            ]),
            Hash::new_from_chunks(&[
                b"state merge finality post state".as_slice(),
                height_bytes.as_slice(),
            ]),
            Hash::new_from_chunks(&[
                b"state merge finality ordinary writes".as_slice(),
                height_bytes.as_slice(),
            ]),
            None,
            0,
            u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64"),
            Hash::new(&executed_block_wire),
        )
        .expect("canonical finality execution commitment");
        execution_commitment.merge_carrier = block
            .execution_context()
            .and_then(|context| context.merge_entry.as_ref())
            .map(|reference| {
                iroha_data_model::block::consensus_v2::MergeCarrierCommitmentV1::new(
                    reference.entry_hash,
                )
            });
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical proposal block wire"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: block.header().view_change_index(),
        };
        let signers = (0..keypairs.len())
            .map(|index| u32::try_from(index).expect("fixture signer index fits u32"))
            .collect::<Vec<_>>();
        let mut commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers,
            aggregate_signature: vec![1],
        };
        let preimage = commit_qc
            .signer_preimage(&context, 0)
            .expect("valid finality signer preimage");
        let signatures = keypairs
            .iter()
            .map(|keypair| {
                Signature::try_new(keypair.private_key(), &preimage)
                    .expect("sign finality fixture vote")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        commit_qc.aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate finality fixture votes");
        let validator_set_pops = keypairs
            .iter()
            .map(|keypair| {
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("derive finality fixture proof of possession")
            })
            .collect();
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
        artifact.verify().expect("fixture finality verifies");
        artifact
    }

    let parent_finality = artifact_for_block(state, parent, None, keypairs);
    let _ = state
        .kura
        .store_v2_finality_artifact(&parent_finality)
        .expect("persist exact parent finality");
    let carrier_finality = artifact_for_block(state, carrier, Some(&parent_finality), keypairs);
    let _ = state
        .kura
        .store_v2_finality_artifact(&carrier_finality)
        .expect("persist exact merge-carrier finality");
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture builds one fully certified autonomous execution carrier"
)]
fn autonomous_merge_commit_authorization_fixture(
    seed_expired_axt_replay: bool,
    seed_due_start_effect: bool,
) -> (
    State,
    MergeLedgerEntry,
    SignedBlock,
    Option<AxtHandleReplayKey>,
) {
    let (state, validator_keypairs, commit_keypairs, parent) = configured_single_lane_merge_state();
    if seed_due_start_effect {
        let mut locks = GovernanceLocksForReferendum::default();
        locks.locks.insert(
            (*ALICE_ID).clone(),
            GovernanceLockRecord {
                owner: (*ALICE_ID).clone(),
                amount: Quantity::from(1_u32),
                slashed: Quantity::zero(),
                expiry_height: 1,
                direction: 0,
                duration_blocks: 0,
                custody: Some(GovernanceLockCustody {
                    escrowed: false,
                    asset_definition_id: state.gov.voting_asset_id.clone(),
                    bond_escrow_account: state.gov.bond_escrow_account.clone(),
                    slash_receiver_account: state.gov.slash_receiver_account.clone(),
                }),
            },
        );
        let mut world = state.world.block();
        world
            .governance_locks
            .insert("autonomous-merge-due-start-effect".to_owned(), locks);
        world.commit();
    }
    let expired_axt_replay_key = seed_expired_axt_replay.then(|| {
        let key = AxtHandleReplayKey::from_parts([0xA7; 32], 1, 1, LaneId::SINGLE);
        let mut replay = state.world.axt_replay_ledger.block();
        replay.insert(
            key,
            AxtReplayRecord {
                dataspace: DataSpaceId::UNIVERSAL,
                used_slot: 0,
                retain_until_slot: 0,
            },
        );
        replay.commit();
        key
    });

    let tag = 0x6A;
    let entrypoint = queue_plan_entrypoint_for_state_test(&state, tag);
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (binding, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        1,
        tag,
    );
    {
        let mut world = state.world.block();
        world.accounts.insert(
            entrypoint.authority().clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.commit();
    }
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let source = autonomous_merge_source_for_queue_plan_admission_test(
        &state,
        &binding,
        entrypoint,
        routing_plan,
        &commit_keypairs,
    );
    let application_header = BlockHeader::new(
        nonzero!(2_u64),
        Some(parent.hash()),
        None,
        None,
        u64::try_from(parent.header().creation_time().as_millis())
            .expect("fixture parent time fits u64")
            .saturating_add(1),
        0,
    );
    let batch = state
        .build_merge_execution_batch_from_source_prefix(1, application_header, vec![source])
        .expect("fixture source produces a canonical autonomous execution batch");
    let lifecycle = state.lane_consensus_lifecycle_snapshot();
    let active_lanes = lifecycle
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| MergeLaneBinding {
            lane_id: lane.id,
            dataspace_id: lane.dataspace_id,
            lane_config_hash: merge_lane_config_hash(lane),
            incarnation: lifecycle.incarnations[&lane.id],
            activation_height: lifecycle.activation_heights[&lane.id].saturating_add(1),
        })
        .collect::<Vec<_>>();
    let incarnation_entries = active_lanes
        .iter()
        .map(
            |lane| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                lane_id: lane.lane_id,
                incarnation: lane.incarnation,
            },
        )
        .collect::<Vec<_>>();
    let candidate = crate::merge::MergeLedgerCandidate {
        version: crate::merge::MergeLedgerCandidate::VERSION,
        epoch_id: 1,
        view: 0,
        carrier_height: 2,
        carrier_parent_hash: parent.hash(),
        lane_catalog_hash: merge_lane_catalog_hash(&lifecycle.nexus.lane_catalog),
        incarnation_root: LaneLifecycleParameterV1::incarnation_root(&incarnation_entries),
        activation_root: crate::merge::merge_activation_root(&active_lanes),
        active_lanes,
        lane_snapshots: Vec::new(),
        execution_batch: Some(batch),
        lane_drain_certificates: Vec::new(),
        queue_plan_admissions: Vec::new(),
        global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
    };
    state
        .validate_merge_candidate_for_global_round(&candidate, &parent.header(), 0)
        .expect("fixture autonomous execution candidate is valid");
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    let carrier = certified_merge_carrier_after(&parent, &entry);
    state
        .kura
        .store_block_with_merge_entry(Arc::new(carrier.clone()), &entry)
        .expect("persist exact autonomous execution carrier");
    persist_merge_carrier_finality_chain_for_state_test(
        &state,
        &parent,
        &carrier,
        &commit_keypairs,
    );
    (state, entry, carrier, expired_axt_replay_key)
}

fn staged_autonomous_merge_commit_block<'state>(
    state: &'state State,
    entry: &MergeLedgerEntry,
    carrier: &SignedBlock,
) -> StateBlock<'state> {
    let mut state_block = state
        .block_with_certified_merge_entry(carrier.header().clone(), entry)
        .expect("certified autonomous execution must stage on its exact carrier");
    assert!(
        state_block
            .canonical_wsv_merge_commit_authorization
            .is_some(),
        "successful re-execution must mint canonical WSV commit authorization"
    );
    stage_exact_empty_autonomous_carrier_membership_for_pre_vote(&mut state_block);
    let (time_entrypoints, time_hashes, time_results) =
        state_block.execute_time_triggers(&carrier.header());
    assert!(time_entrypoints.is_empty());
    assert!(time_hashes.is_empty());
    assert!(time_results.is_empty());
    state_block
        .validate_staged_merge_execution_authorization()
        .expect("pre-vote authorization must bind deterministic carrier events");
    let committed = ValidBlock::new_unverified_for_tests(carrier.clone())
        .commit_unchecked()
        .unpack(|_| {});
    let topology = state.commit_topology_snapshot();
    let _events = state_block.apply_without_execution(&committed, topology);
    assert!(
        state_block
            .canonical_carrier_commit_metadata_authorization
            .is_some(),
        "exact finalized carrier application must mint metadata authorization"
    );
    state_block
}

fn stage_exact_empty_autonomous_carrier_membership_for_pre_vote(state_block: &mut StateBlock<'_>) {
    let height = autonomous_carrier_transaction_height(state_block);
    state_block
        .transactions
        .insert_block(std::collections::HashSet::new(), height);
}

fn autonomous_carrier_transaction_height(state_block: &StateBlock<'_>) -> NonZeroUsize {
    usize::try_from(state_block._curr_block.height().get())
        .ok()
        .and_then(NonZeroUsize::new)
        .expect("autonomous carrier height fits canonical transaction storage")
}

struct ExactTestStateBlockCommitAuthorization {
    carrier_block_hash: HashOf<BlockHeader>,
    execution_reference: iroha_data_model::block::CertifiedMergeLedgerReference,
    lane_count: usize,
}

impl StateBlockCommitAuthorization for ExactTestStateBlockCommitAuthorization {
    fn consume_for_state_commit(
        self: Box<Self>,
        carrier_block_hash: HashOf<BlockHeader>,
        staged_merge_entry: Option<&MergeLedgerEntry>,
    ) -> Result<(), String> {
        let entry = staged_merge_entry
            .filter(|entry| entry.execution_batch.is_some())
            .ok_or_else(|| "test authorization requires one autonomous merge entry".to_owned())?;
        let lane_count = entry
            .execution_batch
            .as_ref()
            .expect("filtered autonomous execution entry")
            .lanes
            .len();
        if carrier_block_hash != self.carrier_block_hash
            || iroha_data_model::block::CertifiedMergeLedgerReference::new(entry)
                != self.execution_reference
            || lane_count != self.lane_count
        {
            return Err("test authorization identity changed before State commit".to_owned());
        }
        Ok(())
    }
}

fn exact_test_state_commit_authorization(
    state_block: &StateBlock<'_>,
) -> Box<dyn StateBlockCommitAuthorization> {
    let entry = state_block
        .staged_merge_entry
        .as_ref()
        .filter(|entry| entry.execution_batch.is_some())
        .expect("fixture State block carries autonomous execution");
    Box::new(ExactTestStateBlockCommitAuthorization {
        carrier_block_hash: state_block._curr_block.hash(),
        execution_reference: iroha_data_model::block::CertifiedMergeLedgerReference::new(entry),
        lane_count: entry
            .execution_batch
            .as_ref()
            .expect("filtered autonomous execution entry")
            .lanes
            .len(),
    })
}

fn commit_staged_autonomous_for_test(
    state_block: StateBlock<'_>,
) -> Result<(), TransactionsBlockError> {
    let authorization = exact_test_state_commit_authorization(&state_block);
    state_block.commit_with_state_commit_authorization(authorization)
}
